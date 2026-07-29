.PHONY: all doc serve clean

SITE_DIR = site

# Build everything: Rust API docs + language book
all: doc

doc:
	cargo doc --no-deps
	mdbook build docs/
	rm -rf docs/book/api
	cp -r target/doc docs/book/api

# Serve the full documentation site locally
serve: doc
	python3 -m http.server 8000 -d docs/book

# Rebuild and open in browser
open: doc
	xdg-open docs/book/index.html 2>/dev/null || open docs/book/index.html 2>/dev/null || true

# Clean all generated documentation
clean:
	cargo clean
	rm -rf docs/book
