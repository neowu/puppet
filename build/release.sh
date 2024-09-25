#!/bin/bash -xe
cargo build --release

sudo cp ./target/release/puppet /usr/local/bin
puppet completion | tee ~/.config/fish/completions/_puppet

mkdir -p ~/.config/puppet
cp env/llm.json ~/.config/puppet/
cp env/tts.json ~/.config/puppet/
