#!/bin/bash

# Script to update wallet logo in extension files

ICON_FILE="icons/icon48.png"
TEMP_FILE="/tmp/unruggable_logo_base64.txt"

echo "🎨 Converting logo to base64..."
base64 -i "$ICON_FILE" -o "$TEMP_FILE"

# Create the data URI
ICON_DATA_URI="data:image/png;base64,$(cat $TEMP_FILE | tr -d '\n')"

echo "✅ Base64 size: $(wc -c < $TEMP_FILE) bytes"

# Update inject.js
echo "📝 Updating inject.js..."
perl -i -pe "s|this\.icon = 'data:image/[^']+';|this.icon = '$ICON_DATA_URI';|g" inject.js

# Update wallet-standard.js (if it exists)
if [ -f "wallet-standard.js" ]; then
    echo "📝 Updating wallet-standard.js..."
    perl -i -pe "s|this\.icon = 'data:image/[^']+';|this.icon = '$ICON_DATA_URI';|g" wallet-standard.js
fi

# Update wallet-standard-impl.js
if [ -f "wallet-standard-impl.js" ]; then
    echo "📝 Updating wallet-standard-impl.js..."
    # This one uses a different pattern
    perl -i -pe "s|icon: '[^']+',|icon: '$ICON_DATA_URI',|g" wallet-standard-impl.js
fi

echo "✅ Logo updated in all files!"
echo "🔄 Reload the extension to see your new logo"

# Cleanup
rm -f "$TEMP_FILE"
