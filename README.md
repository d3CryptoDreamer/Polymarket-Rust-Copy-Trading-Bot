# Polymarket Copy Trading Bot

Rust bot that mirrors trades from chosen Polymarket wallets. It connects to Polymarket’s real-time WebSocket, watches a target wallet’s activity, and places matching market orders (FAK) on the CLOB using your funded account.

## Features

- Real-time trade feed via WebSocket (`wss://ws-live-data.polymarket.com`)
- Copies **BUY** trades from a configurable target wallet
- Configurable size multiplier and min/max order size (USDC)
- Sets USDC and Conditional Token (ERC1155) approvals on startup
- Optional auto-reconnect

## Requirements

- [Rust](https://rustup.rs/) (2021 edition)
- Polygon wallet with USDC for trading
- Polymarket CLOB-compatible private key

## Setup

1. **Clone and build**

   ```bash
   git clone <repo-url>
   cd polymarket-rust-copy-trading-bot
   cargo build --release
   ```

2. **Environment**

   Create a `.env` in the project root:

   ```env
   PRIVATE_KEY=0x...   # Polygon private key (with leading 0x)
   MULTIPLIER=1         # Optional; default 1 (1 = same size as target)
   ```

   - `PRIVATE_KEY` – Used to sign orders and set approvals. Must hold USDC on Polygon.
   - `MULTIPLIER` – Scale vs target size (e.g. `0.5` = 50% of their size).

3. **Target wallet**

   The wallet to copy is set in code: edit `TARGET_WALLET` in `src/main.rs` to the Polymarket proxy wallet (or name) you want to follow.

4. **Trading switch**

   Trading is off by default. Set `enable_trading = true` in `src/main.rs` when you are ready to place real orders.

## Run

```bash
cargo run --release
```

On startup the bot will set USDC and CTF approvals, then connect to the WebSocket and subscribe to trade activity. Use Ctrl+C to stop.

## Order sizing

- Order size = `price × size × MULTIPLIER` from the target’s trade.
- Result is clamped between **1** and **4** USDC (see `min_value` / `max_value` in `src/main.rs` to change).

## Project layout

- `src/main.rs` – Entrypoint, CLOB client, WebSocket callbacks, order logic
- `src/real_time_data_client/` – WebSocket client and message types
- `src/util.rs` – USDC and Conditional Token approval helpers

## License

Use and modify at your own risk. Not affiliated with Polymarket.
