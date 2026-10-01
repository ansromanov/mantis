#!/usr/bin/env bash
set -euo pipefail

{
    git diff --name-only origin/main...HEAD
    git diff --name-only
    git diff --name-only --cached
} | sort -u
