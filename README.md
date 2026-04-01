# text-to-cypher

A comparative study of two Neo4j query generation architectures:

1. **Direct Translation** — natural language goes straight to Cypher via a pretrained model.
2. **Staged Translation** — natural language is first converted to a controlled natural language (CNL/SQUALL/Sparklis), then to Cypher.

The `t2c` CLI provides tooling to generate evaluation datasets, benchmark models against them, and explore score distributions through an interactive TUI.

## Requirements

- [Rust](https://www.rust-lang.org/)
- [Ollama](https://ollama.com/) running locally
- The models you intend to use pulled into Ollama (see [Models](#models))

Alternatively, build and run with Nix:

```sh
nix run github:c2fc2f/text-to-cypher
```

## Installation

```sh
cargo install --path .
```

This installs the `t2c` binary.

## Models

The pretrain subcommand defaults to `text-to-cypher-gemma3-2025:4b`, a fine-tuned Gemma 3 4B model. To set it up in Ollama:

1. Download the GGUF from HuggingFace:
   ```
   https://huggingface.co/mradermacher/text-to-cypher-Gemma-3-4B-Instruct-2025.04.0-GGUF/resolve/main/text-to-cypher-Gemma-3-4B-Instruct-2025.04.0.Q8_0.gguf
   ```
2. Place the `.gguf` file alongside `data/text-to-cypher-gemma3-2025:4b/Modelfile`.
3. Register it with Ollama:
   ```sh
   ollama create text-to-cypher-gemma3-2025:4b -f data/text-to-cypher-gemma3-2025:4b/Modelfile
   ```

Any Ollama-compatible model works for dataset generation and evaluation. The defaults are chosen deliberately: `ministral-3:3b` generates the dataset and also serves as the semantic validator — using the same model that wrote the natural language questions makes sense, since it has a stronger grasp of their intent. Dataset evaluation is handled by `deepseek-r1:1.5b` to provide an unbiased, external perspective and avoid the "self-grading" bias.

## Usage

### Dataset generation

Generates a dataset of (schema, Cypher query, natural language question) triples. It picks schemas at random from a schema set, asks a generator model to produce a Cypher query, then asks it to produce a natural language question that corresponds to that query.

```sh
t2c dataset generate \
  --schemas data/distinct_schemas.json \
  --output result/datasets/dataset.json \
  --count 500 \
  --threads 4 \
  --generator ministral-3:3b \
  --temperature 0.5
```

| Flag | Description |
|---|---|
| `-s, --schemas` | Path to the JSON file containing graph schemas |
| `-o, --output` | Output path for the generated dataset |
| `-c, --count` | Number of entries to generate |
| `--thread` | Number of parallel generations (default: 1) |
| `-g, --generator` | Ollama model to use (default: `ministral-3:3b`) |
| `-t, --temperature` | Sampling temperature (default: 0.5) |

### Dataset evaluation

Validates an existing dataset by scoring each entry for schema adherence and alignment between the Cypher query and the natural language question. Prints summary statistics on exit.

```sh
t2c dataset evaluate \
  --dataset result/datasets/dataset.json \
  --output result/dataset/evaluate/ \
  --threads 4 \
  --validator deepseek-r1:1.5b
```

| Flag | Description |
|---|---|
| `-d, --dataset` | Path to the dataset file |
| `-o, --output` | Directory where per-entry result files are written |
| `-t, --threads` | Number of parallel validations |
| `-v, --validator` | Ollama model to use as judge (default: `deepseek-r1:1.5b`) |

### Pretrain benchmark

Runs a generator model against each entry in a dataset, scores the generated Cypher against the ground truth using a validator model, and writes one JSON result file per entry.

```sh
t2c pretrain \
  --dataset result/datasets/ataset.json \
  --output result/pretrain/ \
  --threads 4 \
  --generator text-to-cypher-gemma3-2025:4b \
  --validator ministral-3:3b
```

| Flag | Description |
|---|---|
| `-d, --dataset` | Path to the dataset file |
| `-o, --output` | Directory where per-entry result files are written |
| `-t, --threads` | Number of parallel workers |
| `-s, --skip` | Number of entries to skip (useful for resuming) |
| `-g, --generator` | Model that generates Cypher from NL (default: `text-to-cypher-gemma3-2025:4b`) |
| `-v, --validator` | Model that scores the output (default: `ministral-3:3b`) |

Each result file contains the ground-truth entry, the generated Cypher, and a score from 0 to 1.

### Stats viewer

An interactive terminal UI that reads a directory of JSON result files and displays score statistics across four tabs: Summary, Histogram, Distribution, and Files.

```sh
t2c stats --dir results/pretrain/
```

| Flag | Description |
|---|---|
| `--dir` | Directory containing the JSON result files |
| `-d, --depth` | Maximum recursion depth for subdirectories (default: 0) |
| `-f, --field` | JSON field to use as the score (default: `score`) |
| `-b, --buckets` | Number of histogram buckets (default: 10) |

**Keybindings:**

| Key | Action |
|---|---|
| `← →` or `Tab` | Switch tab |
| `↑ ↓` or `j k` | Scroll file list |
| `q` or `Esc` | Quit |

## Data

`data/distinct_schemas.json` contains graph schemas extracted from the [neo4j/text2cypher-2025v1](https://huggingface.co/datasets/neo4j/text2cypher-2025v1) dataset. These schemas serve as the seed for dataset generation.

## Licensing

This repository uses a split-licensing model:

- **Source code** — MIT License (`LICENSE-MIT`)
- **Dataset/Schemas** (`data/distinct_schemas.json`) — Apache License 2.0 (`LICENSE-APACHE`), derived from the `neo4j/text2cypher-2025v1` dataset
