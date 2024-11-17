#[derive(Serialize, Deserialize, Debug)]
pub struct AddressRunes {
    pub ticker: String,
    pub balance: String,
    pub symbol: Option<String>
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AddressPayload {
    pub outputs: Vec<String>,
    pub inscriptions: Vec<String>,
    pub sat_balance: u32,
    pub runes_balances: Vec<AddressRunes>
}

#[derive(Serialize, Deserialize, Debug)]
pub struct StatusPayload {
    pub height: u32,
    pub inscriptions: u32
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ScrapeWallet {
    pub id: u32,
    pub name: String,
    pub address: String
}

// #[derive(Serialize, Deserialize, Debug)]
// pub struct RuneBalance {
//     pub id: u32,
//     pub timestamp: u64,
//     pub ticker: String,
//     pub balance: f32,
//     pub last_balance: f32
// }

// #[derive(Serialize, Deserialize, Hash, Eq, , Debug)]
// "address_index": true,
// "blessed_inscriptions": 76332641,
// "chain": "mainnet",
// "cursed_inscriptions": 472043,
// "height": 864351,``
// "initial_sync_time": {
//   "secs": 59213,
//   "nanos": 979632000
// },
// "inscriptions": 76804684,
// "lost_sats": 0,
// "minimum_rune_for_next_block": "PVHGFEDCAZZ",
// "rune_index": true,
// "runes": 119811,
// "sat_index": false,
// "started": "2024-09-27T17:43:39.291876400Z",
// "transaction_index": false,
// "unrecoverably_reorged": false,
// "uptime": {
//   "secs": 709843,
//   "nanos": 910346200
// }