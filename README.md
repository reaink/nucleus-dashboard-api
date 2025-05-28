# Nucleus Dashboard API

## 🛠️ Installation and Setup

### Prerequisites
- Rust 1.75+ 
- Cargo

### Running the Development Server

```bash
# Clone the project
git clone <your-repo-url>
cd nucleus-dashboard-api

# Run the server
cargo run

# Or run in release mode (better performance)
cargo run --release
```

The server will start at `http://0.0.0.0:4001`.

## 🧪 Testing

```bash
# Run tests
cargo test

# Run tests with output
cargo test -- --nocapture
```