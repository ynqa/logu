# logu

![logu.gif](https://github.com/ynqa/ynqa/blob/master/demo/logu.gif)

*logu* is for extracting patterns from (streaming) unstructured log messages.

For parsing unstructured logs, it uses the parser from
[Drain](https://github.com/logpai/Drain3).
In simple terms, it tokenizes log messages,
builds a tree structure, and groups similar logs into a single cluster,
converting unstructured log data into a format that can be organized and analyzed.

This approach is also used by
[Grafana Loki](https://github.com/grafana/loki/tree/v3.0.0/pkg/pattern/drain).
If you are interested in log parsers themselves,
other methods are summarized at
[logpai/logparser](https://github.com/logpai/logparser),
so please take a look.

## Features

- [x] Extract patterns from streaming log messages
- Enables more detailed analysis
  - [ ] Displays the number of messages included
        and a list of specific examples in the cluster
  - [ ] Identifies attributes such as IP, port

## Installation

### Homebrew

```bash
brew install ynqa/tap/logu
```

### Cargo

```bash
cargo install logu
```

## Examples

```bash
stern --context kind-kind - | logu
```

## Keymap

| Key                 | Action
| :-                  | :-
| <kbd>Ctrl + C</kbd> | Exit `logu`

## Usage

```bash
Usage: logu [OPTIONS]

Options:
      --retrieval-timeout <RETRIEVAL_TIMEOUT_MILLIS>
          Timeout to read a next line from the stream in milliseconds. [default: 10]
      --render-interval <RENDER_INTERVAL_MILLIS>
          Interval to render the list in milliseconds. [default: 100]
      --train-interval <TRAIN_INTERVAL_MILLIS>
          [default: 10]
      --cluster-size-th <CLUSTER_SIZE_TH>
          Threshold to filter out small clusters. [default: 0]
      --max-clusters <MAX_CLUSTERS>

      --max-node-depth <MAX_NODE_DEPTH>
          [default: 2]
      --sim-th <SIM_TH>
          [default: 0.4]
      --max-children <MAX_CHILDREN>
          [default: 100]
      --param-str <PARAM_STR>
          [default: <*>]
  -h, --help
          Print help (see more with '--help')
  -V, --version
          Print version
```
