# Reminder Push Notifications

The app can receive reminder push notifications when configured.

## Setup

1. **Firebase** (required for push):
   ```bash
   flutterfire configure
   ```
   Creates `lib/firebase_options.dart` and `android/app/google-services.json`.

2. **Settings** → set **Reminder push server** to your http_server URL
   - Example: `https://your-pinggy-tunnel:1235` or `http://10.0.2.2:1235` (Android emulator to host)

3. Grant notification permission when prompted.

## Without Firebase

The app runs without push if Firebase is not configured. Reminders will still be stored; they just won't push to the device until you complete the setup.
