#!/bin/bash

REGISTRY="crates-io"

echo "发布 unis-macros..."
(cd unis-macros && cargo publish --registry $REGISTRY)

echo "发布 unis-utils..."
(cd unis-utils && cargo publish --registry $REGISTRY)

echo "发布 unis..."
(cd unis && cargo publish --registry $REGISTRY)

echo "发布 unis-kafka..."
(cd unis-kafka && cargo publish --registry $REGISTRY)
