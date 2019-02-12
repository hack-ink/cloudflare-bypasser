## Intro

**cloudflare-bypasser**

Inspired by python module [cloudflare-scrape](https://github.com/Anorov/cloudflare-scrape)

## Require

- **Node.js**

## Example

```rust
extern crate cloudflare_bypasser;
extern crate reqwest;

fn main() {
    const WEBSITE: &'static str = "https://example.com";

    // quick start
    let mut bypasser = {
        cloudflare_nypasser::Bypasser::new()
    };

    // customize
    let mut bypasser = {
        cloudflare_bypasser::Bypasser::new()
            .retry(100)                     // retry times, it might be 10000, depends on your network environment, default 30
            .proxy("http://127.0.0.1:1087") // use proxy, default None
            .random_user_agent(true)        // use random user agent, default false
            .user_agent("Mozilla/5.0")      // specify user agent manually, default ""
            .wait(5);                       // cloudflare's waiting time, but in my test it can be 0, default 0
    };                           

    // to pass the verify both of the cookie and user agent are needed
    let (cookie, user_agent);
        loop {
            if let Ok((c, ua)) =  bypasser.bypass(WEBSITE) {
                cookie = c;
                user_agent = ua;
                break;
            }
        }
    
    // use reqwest without proxy
    {
        // 1
        {
            let client = {
                let headers = {
                    let mut h = reqwest::header::HeaderMap::new();
                    h.insert(reqwest::header::COOKIE, cookie);
                    h.insert(reqwest::header::USER_AGENT, user_agent);
                    h
                };
                
                reqwest::ClientBuilder::new()
                    .default_headers(headers)
                    .build()
                    .unwrap()
            };
                
            let text = client.get(WEBSITE)
                .send()
                .unwrap()
                .text()
                .unwrap();
            println!("{}", text);
        }
        
        // 2
        {
            let text = reqwest::Client::new()
                .get(WEBSITE)
                .header(reqwest::header::COOKIE, cookie)
                .header(reqwest::header::USER_AGENT, user_agent)
                .send()
                .unwrap()
                .text()
                .unwrap();
            println!("{}", text);
        }
    }
    
    // use reqwest with proxy
    {
        let client = {
            let headers = {
                let mut h = reqwest::header::HeaderMap::new();
                h.insert(reqwest::header::COOKIE, cookie);
                h.insert(reqwest::header::USER_AGENT, user_agent);
                h
            };
            
            reqwest::ClientBuilder::new()
                .default_headers(headers)
                .proxy(reqwest::Proxy::all("http://127.0.0.1:1087").unwrap())
                .build()
                .unwrap()
        };
            
        let text = client.get(WEBSITE)
            .send()
            .unwrap()
            .text()
            .unwrap();
        println!("{}", text);
    }
}
```
