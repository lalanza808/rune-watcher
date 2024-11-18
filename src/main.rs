#[macro_use] extern crate serde_derive;
extern crate tokio;
extern crate reqwest;
extern crate rusqlite;
extern crate serde_json;
extern crate dotenv;
mod types;

use types::{AddressPayload, StatusPayload, ScrapeWallet};
use reqwest::{Client, header, Response};
use rusqlite::Connection;
use tokio::time::{sleep, Duration};
use std::path::Path;
use std::fs::File;
use std::time::{SystemTime, UNIX_EPOCH};
use std::process::Command;
use std::io::{BufReader, BufRead};

static DB_PATH: &str = "db.sqlite";
static WALLETS_PATH: &str = "wallets.csv";
static AUDIO_PATH: &str = "sound.mp3";

fn get_time() -> u64 {
    let start_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Failed to get start time");
    let seconds: u64 = start_time.as_secs();
    return seconds
}

fn get_scrape_list(connection: &Connection) -> Vec<ScrapeWallet> {
    let mut statement = connection.prepare("SELECT * FROM scrape").unwrap();
    let mut rows = statement.query([]).unwrap();
    let mut wallets: Vec::<ScrapeWallet> = Vec::new();
    while let Some(row) = rows.next().unwrap() {
        let wallet: ScrapeWallet = ScrapeWallet {
            id: row.get_unwrap::<usize, u32>(0),
            name: row.get_unwrap::<usize, String>(1),
            address: row.get_unwrap::<usize, String>(2)
        };
        wallets.push(wallet);
    }
    return wallets
}

fn get_last_balance(connection: &Connection, address: String, rune: String) -> f32 {
    let compare_statement: &str = r#"
        SELECT
            LAG (balance, 1, 0) OVER (ORDER BY timestamp) last_balance
        FROM balances
        WHERE
            address = ?1
            and ticker = ?2
        ORDER BY timestamp DESC
        LIMIT 1;"#;
    let mut statement = connection.prepare(compare_statement).unwrap();
    let mut rows = statement.query([address.as_str(), rune.as_str()]).unwrap();
    return rows.next().unwrap().unwrap().get_unwrap::<usize, f32>(0)
}

fn get_last_height(connection: &Connection) -> Option<u32> {
    let compare_statement: &str = r#"
        SELECT
            height
        FROM state
        ORDER BY height DESC
        LIMIT 1;"#;
    let mut statement = connection.prepare(compare_statement).unwrap();
    let mut rows = statement.query([]).unwrap();
    let last_height = rows.next().unwrap();
    return match last_height {
        Some(h) => {
            return match Some(h.get_unwrap::<usize, u32>(0)) {
                Some(lh) => Some(lh),
                None => None
            }
        },
        None => None
    }
}

fn update_scrape_list(connection: &Connection) {
    match File::open(WALLETS_PATH) {
        Ok(file) => {
            let reader = BufReader::new(file);
            for line in reader.lines() {
                let line = line.expect("Error reading line");
                let entries: Vec<&str> = line.split(",").collect();
                let query = connection.execute(
                    "INSERT INTO scrape (name, address) VALUES (?1, ?2)",
                    (entries[0], entries[1])
                );
                match query {
                    Ok(_) => println!("[+] Added new wallet {} ({})", entries[0], entries[1]),
                    Err(_) => ()
                }
            }
        },
        _ => println!("[!] Scrape list at {} was not found. Skipping.", WALLETS_PATH)
    }
}

fn create_db() -> Connection {
    if !Path::new(DB_PATH).exists() {
        println!("[.] Creating new sqlite database at {}", DB_PATH);
        let connection: Connection = Connection::open(DB_PATH).unwrap();
        connection.execute(r#"
            CREATE TABLE scrape (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                address TEXT NOT NULL UNIQUE
            );"#,
            ())
            .unwrap();
        connection.execute(r#"
            CREATE TABLE balances (
                id INTEGER PRIMARY KEY,
                timestamp INTEGER NOT NULL,
                address TEXT NOT NULL,
                ticker TEXT NOT NULL,
                symbol TEXT NOT NULL,
                balance FLOAT NOT NULL
            );"#,
            ())
            .unwrap();
        connection.execute(r#"
            CREATE TABLE state (
                id INTEGER PRIMARY KEY,
                height INTEGER NOT NULL,
                inscriptions INTEGER NOT NULL
            );"#,
            ())
            .unwrap();
        return connection
    } else {
        let connection: Connection = Connection::open(DB_PATH).unwrap();
        return connection
    }
}

async fn fetch_payload(http_client: &Client, url: &str) -> serde_json::Value {
    let response: Response = http_client.get(url)
        .send()
        .await
        .unwrap();
    return response.json().await.unwrap();
}

async fn post_webhook(http_client: &Client, title: String, fields: serde_json::Value) {
    dotenv::dotenv().ok();
    let webhook_url = dotenv::var("WEBHOOK");
    match webhook_url {
        Ok(url) => {
            let raw_body: serde_json::Value = serde_json::json!({
                "embeds": [
                    {
                        "title": title,
                        "description": "Activity found in wallets being watched.",
                        "color": 15258703,
                        "fields": fields
                    }
                ]
            });
            let _ = http_client.post(url)
                .json(&raw_body)
                .send()
                .await
                .unwrap();
        },
        Err(_) => ()
    }

}

#[tokio::main]
async fn main() {


    loop {

        let connection: Connection = create_db();
        let _ = update_scrape_list(&connection);
        let url = "http://127.0.0.1:8080";
        let mut headers = header::HeaderMap::new();
        headers.insert("Accept", header::HeaderValue::from_static("application/json"));
        let client = Client::builder()
            .default_headers(headers)
            .build()
            .unwrap();

        let res: serde_json::Value = fetch_payload(&client, format!("{}/status", url).as_str()).await;
        let status: StatusPayload = serde_json::from_value(res).unwrap();
        let last_height: Option<u32> = get_last_height(&connection);
        let ready_to_scan = match last_height {
            Some(lh) => {
                if status.height <= lh {
                    // println!("[!] Last checked block height {} matches current block height, waiting", lh);
                    false
                } else {
                    true
                }
            },
            None => {
                true
            }
        };

        if ready_to_scan {
            // println!("[+] Scanning wallets.");
            let _ = &connection.execute(
                "INSERT INTO state (height, inscriptions) VALUES (?1, ?2)",
                (
                    status.height,
                    status.inscriptions
                )
            ).unwrap();

            // println!("[.] Checking wallets");
            let wallets: Vec<ScrapeWallet> = get_scrape_list(&connection);
            let mut deltas: u32 = 0;
            let mut message_fields: Vec<serde_json::Value> = Vec::new();
            for wallet in wallets {
                let timestamp: u64 = get_time();
                let mut rune_balances: Vec<String> = Vec::new();
                let response: serde_json::Value = fetch_payload(&client, format!("{}/address/{}", url, wallet.address).as_str()).await;
                let payload: AddressPayload = serde_json::from_value(response).unwrap();
                for rune in payload.runes_balances {
                    let _ = &connection.execute(
                        "INSERT INTO balances (timestamp, address, ticker, symbol, balance) VALUES (?1, ?2, ?3, ?4, ?5)",
                        (
                            timestamp,
                            wallet.address.as_str(),
                            rune.ticker.as_str(),
                            rune.symbol.unwrap_or_default(),
                            rune.balance.as_str()
                        )
                    ).unwrap();
                    let last_balance: f32 = get_last_balance(&connection, wallet.address.clone(), rune.ticker.clone());
                    let balance: f32 = rune.balance.clone().parse().unwrap();
                    let delta: f32 = balance - last_balance;
                    if delta != 0.0 {
                        deltas += 1;
                        println!("> {} ({}): {} {}", wallet.address.clone(), wallet.name.clone(), delta, rune.ticker.clone());
                        if delta > 0.0 {
                            rune_balances.push(format!("+{} [{}](https://magiceden.io/runes/{})", delta, rune.ticker, rune.ticker));
                        } else {
                            rune_balances.push(format!("{} [{}](https://magiceden.io/runes/{})", delta, rune.ticker, rune.ticker));
                        }
                    }
                }
                if rune_balances.len() > 0 {
                    message_fields.push(serde_json::json!({
                        "name": format!("{} ({})", wallet.address, wallet.name),
                        "value": rune_balances.join("\n")
                    }))
                }
            }
            if deltas > 0 {
                let _ = Command::new("mplayer")
                    .args([AUDIO_PATH])
                    .output();
            }
            if message_fields.len() > 0 {
                let _ = post_webhook(
                    &client,
                    format!("Block {}", status.height),
                    serde_json::Value::Array(message_fields)
                ).await;
            }
        }

        // println!("[.] Sleeping.");
        sleep(Duration::from_millis(10000)).await;
    }

}
