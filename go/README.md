# Nefu Go Version

The Go implementation of Nefu, packaging web projects into desktop executables.

## Requirements

- Go 1.21+
- Windows / macOS / Linux

## Compile

```bash
cd go
go mod tidy
go build -o nefu.exe .
```

## Usage

```bash
# Initialize a project
./nefu init my-app

# Start the dev server (hot reload)
./nefu start

# Package into an executable
./nefu build
```

## Configuration

Create a `main.nefu` file (TOML format) in the project root:

```toml
[app]
name = "My App"
version = "1.0.0"
width = 1024
height = 768

[build]
entry = "index.html"
output = "dist/app.exe"
encrypt = true
```

## .nc Component Language

Nefu supports `.nc` files (a JSON-based UI description language) with 30+ built-in Bootstrap 5 components.

See [docs/manual.md](../docs/manual.md) for detailed documentation.

## License

MIT License
