extern crate base64;
extern crate fake_useragent;
extern crate regex;
extern crate reqwest;
extern crate url;

// --- std ---
use std::time::Duration;
// --- external ---
use reqwest::{
    blocking::ClientBuilder,
    header::{HeaderValue, COOKIE, REFERER, SET_COOKIE, USER_AGENT},
};

pub struct Bypasser<'a> {
    wait: u8,
    retry: u32,
    proxy: Option<&'a str>,
    user_agent: String,
    client: reqwest::blocking::Client,
    user_agents: Option<fake_useragent::UserAgents>,
}

impl<'a> Default for Bypasser<'a> {
    fn default() -> Self {
        Bypasser {
            wait: 3,
            retry: 1,
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
}

impl<'a> Bypasser<'a> {
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
            self.user_agents = Some(
                fake_useragent::UserAgentsBuilder::new()
                    .cache(false)
                    .set_browsers(fake_useragent::Browsers::new().set_chrome().set_firefox().set_safari())
                    .build(),
            );
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
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(30));
        if let Some(address) = self.proxy {
            client_builder = client_builder.proxy(reqwest::Proxy::all(address).unwrap());
        }
        self.client = client_builder.build().unwrap();
        self
    }

    fn parse_challenge(html: &str) -> Vec<(String, String)> {
        regex::Regex::new(r#"name="(r|jschl_vc|pass)"(?: [^<>]*)? value="(.+?)""#)
            .unwrap()
            .captures_iter(html)
            .map(|caps| (caps[1].to_owned(), caps[2].to_owned()))
            .collect()
    }

    fn parse_js(html: &str, domain: &str) -> String {
        // --- external ---
        use regex::Regex;

        let challenge = &Regex::new(
            r#"setTimeout\(function\(\)\{\s+(var s,t,o,p,b,r,e,a,k,i,n,g,f.+?\r?\n[\s\S]+?a\.value =.+?)\r?\n"#,
        )
        .unwrap()
        .captures(html)
        .unwrap()[1];
        let inner_html = if let Some(caps) = Regex::new(r#"<div(?: [^<>]*)? id="cf-dn.*?">([^<>]*)"#)
            .unwrap()
            .captures(html)
        {
            caps[1].to_owned()
        } else {
            String::new()
        };
        format!(
            r#"
                var document = {{
                    createElement: function () {{
                        return {{ firstChild: {{ href: "http://{}/" }} }}
                    }},
                    getElementById: function () {{
                        return {{"innerHTML": "{}"}};
                    }}
                }};
                {}; process.stdout.write(a.value);
            "#,
            domain, inner_html, challenge
        )
    }

    fn run_js(js: &str) -> String {
        String::from_utf8(
            std::process::Command::new("node")
                .args(&["-e", js])
                .output()
                .unwrap()
                .stdout,
        )
        .unwrap()
    }

    fn request_challenge(&mut self, url: &str) -> (String, String, HeaderValue, String) {
        self.build_client();
        if let Some(ref user_agents) = self.user_agents {
            self.user_agent = user_agents.random().to_string();
        }
        loop {
            match self.client.get(url).header(USER_AGENT, self.user_agent.as_str()).send() {
                Ok(resp) => {
                    let url = resp.url().as_str().to_owned();
                    let cookie = resp.headers()[SET_COOKIE].to_owned();
                    match resp.text() {
                        Ok(text) => {
                            let path = regex::Regex::new(r#"id="challenge-form" action="([^"]*)""#)
                                .unwrap()
                                .captures(&text)
                                .unwrap()[1]
                                .into();
                            return (text, url, cookie, path);
                        }
                        Err(e) => println!("At request_challenge() text(), {:?}", e),
                    }
                }
                Err(e) => println!("At, request_challenge() send(), {:?}", e),
            }
        }
    }

    fn solve_challenge(
        &mut self,
        url: &str,
        cookie: &HeaderValue,
        referer: &str,
        params: &[(String, String)],
    ) -> Result<(HeaderValue, HeaderValue), &str> {
        let mut retry = 0u32;
        loop {
            match self
                .client
                .post(url)
                .header(COOKIE, cookie)
                .header(REFERER, referer)
                .header(USER_AGENT, self.user_agent.as_str())
                .form(params)
                .send()
            {
                Ok(resp) => {
                    if resp.headers().contains_key(SET_COOKIE) {
                        return Ok((resp.headers()[SET_COOKIE].to_owned(), self.user_agent.parse().unwrap()));
                    }
                }
                Err(e) => println!("{:?}", e),
            }

            retry += 1;
            if retry == self.retry {
                return Err("reach max retries");
            }
        }
    }

    pub fn bypass(&mut self, url: &str) -> Result<(HeaderValue, HeaderValue), &str> {
        let (html, referer, cookie, path) = self.request_challenge(url);

        let (challenge_url, domain) = {
            let url = url::Url::parse(url).unwrap();
            let domain = url.domain().unwrap().to_owned();

            (format!("{}://{}{}", url.scheme(), domain, path), domain)
        };
        let params = {
            let mut p = Bypasser::parse_challenge(&html);
            p.push((
                String::from("jschl_answer"),
                Bypasser::run_js(&Bypasser::parse_js(&html, &domain)),
            ));

            p
        };

        std::thread::sleep(Duration::from_secs(self.wait as u64));

        self.solve_challenge(&challenge_url, &cookie, &referer, &params)
    }
}
