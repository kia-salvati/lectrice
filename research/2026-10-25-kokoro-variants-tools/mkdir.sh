#!/bin/sh
# usage: mkdir.sh <name> <model file>  -> ./v/<name>/ (or $S/v) with model.onnx + symlinked shared files
S=${S:-$(pwd)}
M=/home/kia/Desktop/learning/lectrice/models/kokoro-multi-lang-v1_0
d=$S/v/$1; rm -rf $d; mkdir -p $d
for f in voices.bin tokens.txt lexicon-us-en.txt espeak-ng-data; do ln -s $M/$f $d/$f; done
ln -s "$(realpath $2)" $d/model.onnx
