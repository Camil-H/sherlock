<p align="center">
  <h1 align="center">Sherlock</h1>
  <p align="center">
    <strong>LLM API Traffic Inspector & Token Usage Dashboard</strong>
  </p>
  <p align="center">
    <img src="https://img.shields.io/badge/python-3.10+-3776AB?logo=python&logoColor=white" alt="Python">
    <img src="https://img.shields.io/badge/license-MIT-green.svg" alt="License">
    <img src="https://img.shields.io/badge/platform-macOS%20%7C%20Linux-lightgrey.svg" alt="Platform">
    <img src="https://img.shields.io/badge/proxy-mitmproxy-orange.svg" alt="mitmproxy">
    <img src="https://img.shields.io/badge/LLM-Anthropic%20Claude-blueviolet.svg" alt="Anthropic">
  </p>
  <p align="center">
    <a href="#installation">Installation</a> •
    <a href="#quick-start">Quick Start</a> •
    <a href="#features">Features</a> •
    <a href="#commands">Commands</a> •
    <a href="#contributing">Contributing</a>
  </p>
</p>

---

Sherlock is a transparent proxy that intercepts HTTPS traffic to LLM APIs and displays real-time token usage in a beautiful terminal dashboard. Track your AI costs, debug prompts, and monitor context window usage across your development session.

## Why Sherlock?

- **Track Token Usage**: See exactly how many tokens each request consumes
- **Monitor Context Windows**: Visual fuel gauge shows cumulative usage against your limit
- **Debug Prompts**: Automatically saves every prompt as markdown and JSON for review
- **Zero Code Changes**: Works with any tool that respects proxy environment variables

## Installation

```bash
# Clone the repository
git clone https://github.com/jmuncor/sherlock.git
cd sherlock

# Install in development mode
pip install -e .
```

### Requirements

- Python 3.10+
- Node.js (for intercepting Node.js applications like Claude Code)

## Quick Start

### 1. Start Sherlock

```bash
sherlock
```

On first run, Sherlock will:
- Generate the mitmproxy CA certificate
- Prompt you to install it in your system trust store
- Ask where to save intercepted prompts

### 2. Run Your LLM Tools

In a separate terminal, use Sherlock to proxy your commands:

```bash
# For Claude Code
sherlock claude

# For any command
sherlock run --node your-llm-tool
```

That's it! Watch the dashboard update in real-time as you interact with LLM APIs.

## Features

### Live Terminal Dashboard

```
┌─────────────────────────────────────────────────────────────┐
│  🔍 SHERLOCK - LLM Traffic Inspector                        │
├─────────────────────────────────────────────────────────────┤
│  Context Usage  ████████████░░░░░░░░░░░░░░░░  42%           │
│                 (84,231 / 200,000 tokens)                   │
├─────────────────────────────────────────────────────────────┤
│  Time     Provider    Model                      Tokens     │
│  14:23:01 Anthropic   claude-sonnet-4-20250514   12,847     │
│  14:23:45 Anthropic   claude-sonnet-4-20250514   8,234      │
│  14:24:12 Anthropic   claude-sonnet-4-20250514   15,102     │
├─────────────────────────────────────────────────────────────┤
│  Last Prompt: "Can you help me refactor this function..."   │
└─────────────────────────────────────────────────────────────┘
```

### Prompt Archive

Every intercepted request is saved as:
- **Markdown** - Human-readable format with metadata
- **JSON** - Raw API request body for debugging

### Context Fuel Gauge

Visual progress bar with color-coded warnings:
- 🟢 Green: < 50% usage
- 🟡 Yellow: 50-80% usage
- 🔴 Red: > 80% usage

## Commands

| Command | Description |
|---------|-------------|
| `sherlock` | Start the proxy and dashboard |
| `sherlock start` | Same as above (explicit) |
| `sherlock claude` | Run Claude Code with proxy configured |
| `sherlock run <cmd>` | Run any command with proxy configured |
| `sherlock run --node <cmd>` | Run Node.js app with proxy configured |
| `sherlock check-certs` | Verify CA certificate installation |
| `sherlock install-certs` | Show certificate installation instructions |
| `sherlock env` | Print proxy environment variables |

### Options

```bash
sherlock start [OPTIONS]

Options:
  -p, --port NUM          Proxy port (default: 8080)
  -l, --limit NUM         Token limit for fuel gauge (default: 200000)
  --persist               Save token history across sessions
  --skip-cert-check       Skip certificate verification
```

## How It Works

```
┌──────────────────────────────────────────────────────────────────┐
│                      Your LLM Application                        │
│              (with proxy environment variables)                  │
└─────────────────────────────┬────────────────────────────────────┘
                              │ HTTPS
                              ▼
┌──────────────────────────────────────────────────────────────────┐
│                     mitmproxy (port 8080)                        │
│                   + Sherlock Interceptor                         │
└─────────────────────────────┬────────────────────────────────────┘
                              │ Parsed events
                              ▼
┌──────────────────────────────────────────────────────────────────┐
│                    Sherlock Dashboard                            │
│              Token tracking • Request log • Prompt preview       │
└──────────────────────────────────────────────────────────────────┘
                              │
                              ▼
                    ~/.sherlock/prompts/
                    ├── 2024-01-15_14-23-01_anthropic.md
                    └── 2024-01-15_14-23-01_anthropic.json
```

## Supported Providers

| Provider | Status |
|----------|--------|
| Anthropic (Claude) | ✅ Supported |
| OpenAI | 🔜 Coming soon |
| Google Gemini | 🔜 Coming soon |

## Configuration

### Certificate Setup

Sherlock uses mitmproxy to intercept HTTPS traffic. On first run, it will guide you through installing the CA certificate.

**macOS:**
```bash
sudo security add-trusted-cert -d -r trustRoot \
  -k /Library/Keychains/System.keychain \
  ~/.mitmproxy/mitmproxy-ca-cert.pem
```

**Ubuntu/Debian:**
```bash
sudo cp ~/.mitmproxy/mitmproxy-ca-cert.pem \
  /usr/local/share/ca-certificates/mitmproxy-ca-cert.crt
sudo update-ca-certificates
```

### Environment Variables

For manual proxy configuration:

```bash
export HTTP_PROXY="http://127.0.0.1:8080"
export HTTPS_PROXY="http://127.0.0.1:8080"
export NODE_EXTRA_CA_CERTS="$HOME/.mitmproxy/mitmproxy-ca-cert.pem"
```

Or use the helper:
```bash
eval $(sherlock env)
```

## Contributing

Contributions are welcome! Here's how you can help:

1. **Fork** the repository
2. **Create** a feature branch (`git checkout -b feature/amazing-feature`)
3. **Commit** your changes (`git commit -m 'Add amazing feature'`)
4. **Push** to the branch (`git push origin feature/amazing-feature`)
5. **Open** a Pull Request

### Development Setup

```bash
git clone https://github.com/yourusername/sherlock.git
cd sherlock
python -m venv venv
source venv/bin/activate
pip install -e .
```

### Adding Provider Support

To add support for a new LLM provider:

1. Add the API host to `sherlock/config.py`
2. Create a parser function in `sherlock/parser.py`
3. Update the `parse_request()` function to route to your parser

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

---

<p align="center">
  <em>See what's really being sent to the LLM. Learn. Optimize. Repeat.</em>
</p>

