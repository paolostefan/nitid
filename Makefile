.PHONY: all doc serve clean

SITE_DIR = site

# Build everything: Rust API docs + language book
all: doc

doc:
	cargo doc --no-deps
	mdbook build -d docs src/docs/
	rm -rf docs/api
	cp -r target/doc docs/api

# Serve the full documentation site locally
serve: doc
	python3 -m http.server 8000 -d docs

# Rebuild and open in browser
open: doc
	xdg-open docs/index.html 2>/dev/null || open docs/index.html 2>/dev/null || true

# Clean all generated documentation
clean:
	cargo clean
	rm -rf docs/*
