.PHONY: all doc book api serve open clean

SITE_DIR = site

# Build everything: Rust API docs + language book
all: doc

doc: book api

# Language book only (mdBook). Deterministic: editing one .md touches ~4 files
# in docs/, so this is the target to use while writing prose.
book:
	mdbook build -d docs src/docs/

# Rust API reference only. rustdoc names search-index chunks by content hash, so
# this rewrites ~110 files under docs/api/ on every run — which is why docs/api/
# is gitignored.
api:
	cargo doc --no-deps
	rm -rf docs/api
	cp -r target/doc docs/api

# Serve the full documentation site locally
serve: doc
	python3 -m http.server 8000 -d docs

# Serve just the book, rebuilding on change (no rustdoc churn)
watch:
	mdbook serve -d docs src/docs/

# Rebuild and open in browser
open: doc
	xdg-open docs/index.html 2>/dev/null || open docs/index.html 2>/dev/null || true

# Clean all generated documentation
clean:
	cargo clean
	rm -rf docs/*