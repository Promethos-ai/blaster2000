// Reminder push notification service.
// Registers FCM token with server and handles incoming reminder notifications.

import 'dart:convert';

import 'package:firebase_messaging/firebase_messaging.dart';
import 'package:flutter/foundation.dart';
import 'package:http/http.dart' as http;
import 'package:shared_preferences/shared_preferences.dart';

const String _kFcmTokenPref = 'fcm_token_sent';
const String _kServerBasePref = 'reminder_server_base';

/// Initialize Firebase Messaging, request permission, get token, and register with server.
Future<void> initReminderPush({
  required String serverBaseUrl,
}) async {
  if (kIsWeb) return;

  try {
    final messaging = FirebaseMessaging.instance;

    final settings = await messaging.requestPermission(
      alert: true,
      badge: true,
      sound: true,
    );
    if (settings.authorizationStatus == AuthorizationStatus.denied) return;

    final token = await messaging.getToken();
    if (token == null || token.isEmpty) return;

    final prefs = await SharedPreferences.getInstance();
    final lastSent = prefs.getString(_kFcmTokenPref);
    if (lastSent == token) return;

    final uri = Uri.parse('$serverBaseUrl/api/v1/device-token');
    final resp = await http.post(
      uri,
      headers: {'Content-Type': 'application/json'},
      body: jsonEncode({'token': token}),
    ).timeout(const Duration(seconds: 10));

    if (resp.statusCode == 200) {
      await prefs.setString(_kFcmTokenPref, token);
      await prefs.setString(_kServerBasePref, serverBaseUrl);
    }
  } catch (_) {
    // Silently ignore - push is optional
  }
}

/// Handle foreground messages (show in-app).
void setupForegroundHandler() {
  FirebaseMessaging.onMessage.listen((RemoteMessage message) {
    if (message.notification != null) {
      debugPrint('Reminder (foreground): ${message.notification!.title} ${message.notification!.body}');
    }
  });
}

/// Handle background/terminated message tap.
@pragma('vm:entry-point')
Future<void> firebaseMessagingBackgroundHandler(RemoteMessage message) async {
  // Called when app is in background/terminated
}
