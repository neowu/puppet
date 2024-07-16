#!/bin/bash -e
cargo build --release

sudo cp ./target/release/puppet /usr/local/bin
puppet generate-zsh-completion | sudo tee /usr/local/share/zsh/site-functions/_puppet

mkdir -p ~/.config/puppet
cp env/llm.json ~/.config/puppet/
cp env/tts.json ~/.config/puppet/
