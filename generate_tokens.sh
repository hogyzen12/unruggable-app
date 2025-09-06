#!/bin/bash

# Script to generate src/config/tokens.rs from tokens.json
# Usage: ./generate_tokens.sh tokens.json

set -e  # Exit on any error

# Check if input file is provided
if [ $# -ne 1 ]; then
    echo "Usage: $0 <tokens.json>"
    echo "Example: $0 tokens.json"
    exit 1
fi

INPUT_FILE="$1"
OUTPUT_DIR="src/config"
OUTPUT_FILE="$OUTPUT_DIR/tokens.rs"

# Check if input file exists
if [ ! -f "$INPUT_FILE" ]; then
    echo "Error: Input file '$INPUT_FILE' does not exist"
    exit 1
fi

# Check if jq is available
if ! command -v jq &> /dev/null; then
    echo "Error: jq is required but not installed"
    exit 1
fi

# Create output directory if it doesn't exist
if [ ! -d "$OUTPUT_DIR" ]; then
    echo "Creating directory: $OUTPUT_DIR"
    mkdir -p "$OUTPUT_DIR"
fi

# Create mod.rs if it doesn't exist
MOD_FILE="$OUTPUT_DIR/mod.rs"
if [ ! -f "$MOD_FILE" ]; then
    echo "Creating $MOD_FILE"
    echo "pub mod tokens;" > "$MOD_FILE"
fi

echo "Generating $OUTPUT_FILE from $INPUT_FILE..."

# Start writing the Rust file
cat > "$OUTPUT_FILE" << 'EOF'
use std::collections::HashMap;

// Simple token struct - no dependencies
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifiedToken {
    pub address: String,
    pub name: String,
    pub symbol: String,
    pub logo_uri: String,
    pub tags: Vec<String>,
}

/// Get hardcoded verified tokens
pub fn get_verified_tokens() -> HashMap<String, VerifiedToken> {
    let mut map = HashMap::new();
    
EOF

# Create a temporary file to collect all token entries
TEMP_TOKENS=$(mktemp)

# Process each token in the JSON array and write to temp file
jq -r '.[] | @base64' "$INPUT_FILE" | while read -r token_base64; do
    # Decode the base64 encoded JSON object
    token_json=$(echo "$token_base64" | base64 --decode)
    
    # Extract fields using jq
    id=$(echo "$token_json" | jq -r '.id')
    name=$(echo "$token_json" | jq -r '.name')
    symbol=$(echo "$token_json" | jq -r '.symbol')
    icon=$(echo "$token_json" | jq -r '.icon // ""')
    
    # Extract tags array - handle both arrays and missing tags
    if echo "$token_json" | jq -e '.tags' >/dev/null 2>&1; then
        tags=$(echo "$token_json" | jq -r '[.tags[]? // empty] | map("\"" + . + "\".to_string()") | join(", ")')
    else
        tags=""
    fi
    
    # Escape quotes and special characters in strings for Rust
    name_escaped=$(echo "$name" | sed 's/\\/\\\\/g' | sed 's/"/\\"/g')
    symbol_escaped=$(echo "$symbol" | sed 's/\\/\\\\/g' | sed 's/"/\\"/g')
    icon_escaped=$(echo "$icon" | sed 's/\\/\\\\/g' | sed 's/"/\\"/g')
    
    # Write the token entry to temp file
    cat >> "$TEMP_TOKENS" << EOF
    // $symbol_escaped
    map.insert(
        "$id".to_string(),
        VerifiedToken {
            address: "$id".to_string(),
            name: "$name_escaped".to_string(),
            symbol: "$symbol_escaped".to_string(),
            logo_uri: "$icon_escaped".to_string(),
            tags: vec![$tags],
        },
    );
    
EOF
done

# Append the temp file content to the main output file
cat "$TEMP_TOKENS" >> "$OUTPUT_FILE"

# Clean up temp file
rm -f "$TEMP_TOKENS"

# Close the function
cat >> "$OUTPUT_FILE" << 'EOF'
    map
}
EOF

echo "Successfully generated $OUTPUT_FILE"
echo "Found $(jq length "$INPUT_FILE") tokens in $INPUT_FILE"

# Verify the generated Rust file compiles (optional)
if command -v rustc &> /dev/null; then
    echo "Checking Rust syntax..."
    if rustc --crate-type lib "$OUTPUT_FILE" --emit metadata -o /dev/null 2>/dev/null; then
        echo "✓ Generated Rust file has valid syntax"
    else
        echo "⚠ Warning: Generated Rust file may have syntax issues"
    fi
fi