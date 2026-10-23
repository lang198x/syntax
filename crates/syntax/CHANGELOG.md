# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/lang198x/syntax/releases/tag/lang198x-syntax-v0.1.0) - 2026-10-23

The first release. It provides `TextSize` and `TextRange`, the byte offsets and
spans that every token and node in the tree will be measured in. Arithmetic on
them is checked, and slicing text with a range never panics.
