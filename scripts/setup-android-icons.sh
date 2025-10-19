#!/bin/bash

# Setup Android adaptive icons
echo "Setting up Android adaptive icons..."

# Create all necessary directories
mkdir -p gen/android/app/src/main/res/mipmap-anydpi-v26
mkdir -p gen/android/app/src/main/res/drawable
mkdir -p gen/android/app/src/main/res/mipmap-mdpi
mkdir -p gen/android/app/src/main/res/mipmap-hdpi
mkdir -p gen/android/app/src/main/res/mipmap-xhdpi
mkdir -p gen/android/app/src/main/res/mipmap-xxhdpi
mkdir -p gen/android/app/src/main/res/mipmap-xxxhdpi

# Copy icon.png to all density folders as both launcher and foreground
for density in mdpi hdpi xhdpi xxhdpi xxxhdpi; do
    cp icon.png gen/android/app/src/main/res/mipmap-${density}/ic_launcher.png
    cp icon.png gen/android/app/src/main/res/mipmap-${density}/ic_launcher_round.png
    cp icon.png gen/android/app/src/main/res/mipmap-${density}/ic_launcher_foreground.png
done

# Create adaptive icon XMLs
cat > gen/android/app/src/main/res/mipmap-anydpi-v26/ic_launcher.xml << 'EOF'
<?xml version="1.0" encoding="utf-8"?>
<adaptive-icon xmlns:android="http://schemas.android.com/apk/res/android">
    <background android:drawable="@drawable/ic_launcher_background"/>
    <foreground android:drawable="@mipmap/ic_launcher_foreground"/>
</adaptive-icon>
EOF

cat > gen/android/app/src/main/res/mipmap-anydpi-v26/ic_launcher_round.xml << 'EOF'
<?xml version="1.0" encoding="utf-8"?>
<adaptive-icon xmlns:android="http://schemas.android.com/apk/res/android">
    <background android:drawable="@drawable/ic_launcher_background"/>
    <foreground android:drawable="@mipmap/ic_launcher_foreground"/>
</adaptive-icon>
EOF

# Create background drawable with gradient matching icon.png colors
cat > gen/android/app/src/main/res/drawable/ic_launcher_background.xml << 'EOF'
<?xml version="1.0" encoding="utf-8"?>
<vector xmlns:android="http://schemas.android.com/apk/res/android"
    android:width="108dp"
    android:height="108dp"
    android:viewportWidth="108"
    android:viewportHeight="108">
    <path
        android:fillColor="#FF6B9D"
        android:pathData="M0,0h108v108h-108z" />
</vector>
EOF

echo "✅ Android adaptive icons configured"
