#!/usr/bin/env bash

ls src/*.rs | entr sh -c "
    cargo run

"


