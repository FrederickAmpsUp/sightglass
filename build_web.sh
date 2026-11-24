#!/bin/sh
wasm-pack build --target web --no-gitignore
cp -r assets/ pkg/
