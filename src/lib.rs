extern crate fake_useragent;
extern crate regex;
extern crate reqwest;
extern crate url;

// --- external ---
use reqwest::{
    ClientBuilder,
    header::{COOKIE, REFERER, SET_COOKIE, USER_AGENT, HeaderValue},
};

pub struct Bypasser<'a> {
    wait: u8,
    retry: u32,
    proxy: Option<&'a str>,
    user_agent: String,
    client: reqwest::Client,
    user_agents: Option<fake_useragent::UserAgents>,
}

impl<'a> Bypasser<'a> {
    pub fn new() -> Bypasser<'a> {
        Bypasser {
            wait: 0,
            retry: 30,
            proxy: None,
            user_agent: String::new(),
            client: ClientBuilder::new()
                .danger_accept_invalid_certs(true)
                .danger_accept_invalid_hostnames(true)
                .gzip(true)
                .build()
                .unwrap(),
            user_agents: None,
        }
    }

    pub fn wait(mut self, secs: u8) -> Self {
        self.wait = secs;
        self
    }

    pub fn user_agent(mut self, user_agent: &str) -> Self {
        self.user_agent = user_agent.to_owned();
        self
    }

    pub fn random_user_agent(mut self, flag: bool) -> Self {
        if flag {
            self.user_agents = Some(fake_useragent::UserAgentsBuilder::new()
                .cache(false)
                .set_browsers(fake_useragent::Browsers::new()
                    .set_chrome()
                    .set_firefox()
                    .set_safari())
                .build());
        }
        self
    }

    pub fn proxy(mut self, address: &'a str) -> Self {
        self.proxy = Some(address);
        self
    }

    pub fn retry(mut self, times: u32) -> Self {
        self.retry = times;
        self
    }

    fn build_client(&mut self) -> &mut Self {
        let mut client_builder = ClientBuilder::new()
            .danger_accept_invalid_certs(true)
            .danger_accept_invalid_hostnames(true)
            .gzip(true)
            .redirect(reqwest::RedirectPolicy::none());
        if let Some(address) = self.proxy { client_builder = client_builder.proxy(reqwest::Proxy::all(address).unwrap()); }
        self.client = client_builder.build().unwrap();
        self
    }

    fn parse_challenge(html: &str) -> (String, String) {
        // --- external ---
        use regex::Regex;

        let jschl_vc = Regex::new(r#"name="jschl_vc" value="(\w+)""#)
            .unwrap()
            .captures(html)
            .unwrap()[1]
            .to_owned();
        let pass = Regex::new(r#"name="pass" value="(.+?)""#)
            .unwrap()
            .captures(html)
            .unwrap()[1]
            .to_owned();

        (jschl_vc, pass)
    }

    fn parse_js(html: &str, domain_len: usize) -> String {
        // --- external ---
        use regex::Regex;

        let js = &Regex::new(r#"setTimeout\(function\(\)\{\s+(var s,t,o,p,b,r,e,a,k,i,n,g,f.+?\r?\n[\s\S]+?a\.value =.+?)\r?\n"#)
            .unwrap()
            .captures(html)
            .unwrap()[1];
        let js = &Regex::new(r#"a\.value = (.+ \+ t\.length).+"#)
            .unwrap()
            .replace_all(js, "$1");
        let js = &Regex::new(r#"\s{3,}[a-z](?: = |\.).+"#)
            .unwrap()
            .replace_all(js, "")
            .replace('\n', "")
            .replace("t.length", &domain_len.to_string());

        format!("console.log(require('vm').runInNewContext('{}', Object.create(null), {{timeout: 5000}}));", js)
    }

    fn run_js(js: &str) -> String {
        let mut result = String::from_utf8(
            std::process::Command::new("node")
                .args(&["-e", js])
                .output()
                .unwrap()
                .stdout
        ).unwrap();
        result.pop().unwrap();
        result
    }

    fn request_challenge(&mut self, url: &str) -> (String, String, HeaderValue) {
        self.build_client();
        if let Some(ref user_agents) = self.user_agents { self.user_agent = user_agents.random().to_string(); }
        loop {
            match self.client
                .get(url)
                .header(USER_AGENT, self.user_agent.as_str())
                .send() {
                Ok(mut resp) => {
                    match resp.text() {
                        Ok(text) => {
                            return (
                                text,
                                resp.url().as_str().to_owned(),
                                resp.headers()[SET_COOKIE].to_owned()
                            );
                        }
                        Err(e) => println!("At request_challenge(), text() {:?}", e)
                    }
                }
                Err(e) => println!("At, request_challenge(), send() {:?}", e)
            }
        }
    }

    fn solve_challenge(&mut self, url: &str, cookie: &HeaderValue, referer: &str, query: [&str; 3]) -> Result<(HeaderValue, HeaderValue), &str> {
        let mut retry = 0u32;
        loop {
            match self.client
                .get(url)
                .header(COOKIE, cookie)
                .header(REFERER, referer)
                .header(USER_AGENT, self.user_agent.as_str())
                .query(&[
                    ("jschl_vc", query[0]),
                    ("pass", query[1]),
                    ("jschl_answer", query[2])
                ])
                .send() {
                Ok(resp) => if resp.headers().contains_key(SET_COOKIE) {
                    return Ok((
                        resp.headers()[SET_COOKIE].to_owned(),
                        self.user_agent.parse().unwrap(),
                    ));
                }
                Err(e) => println!("{:?}", e)
            }

            retry += 1;
            if retry == self.retry { return Err("reach max retries"); }
        }
    }

    pub fn bypass(&mut self, url: &str) -> Result<(HeaderValue, HeaderValue), &str> {
        std::thread::sleep(std::time::Duration::from_secs(self.wait as u64));

        let (html, referer, cookie) = self.request_challenge(url);
        let (challenge_url, domain) = {
            let url = url::Url::parse(url).unwrap();
            let domain = url.domain().unwrap().to_owned();
            (format!("{}://{}/cdn-cgi/l/chk_jschl", url.scheme(), domain), domain)
        };
        let (jschl_vc, pass) = Bypasser::parse_challenge(&html);
        let jschl_answer = {
            let js = Bypasser::parse_js(&html, domain.len());
            Bypasser::run_js(&js)
        };

        self.solve_challenge(&challenge_url, &cookie, &referer, [&jschl_vc, &pass, &jschl_answer])
    }
}
