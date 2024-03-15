#!/bin/bash
# get the version from the git tag without v prefix
git describe --tags --abbrev=0 | sed 's/v//'
