#!/bin/bash

(cd unis-macros && cargo publish)
sed -i '/\[patch.crates-io\]/,+2d' Cargo.toml

(cd unis && cargo publish)
(cd unis-kafka && cargo publish)

git checkout Cargo.toml