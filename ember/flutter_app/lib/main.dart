import 'dart:convert';

import 'package:firebase_core/firebase_core.dart';
import 'package:firebase_messaging/firebase_messaging.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_tts/flutter_tts.dart';
import 'package:geolocator/geolocator.dart';
import 'package:http/http.dart' as http;
import 'package:shared_preferences/shared_preferences.dart';
import 'package:speech_to_text/speech_to_text.dart';

import 'ember_client.dart';
import 'ember_client_stub.dart' if (dart.library.io) 'ember_client_stub_unsupported.dart' show askViaHttp, askViaHttpStream;
import 'firebase_options.dart';
import 'reminder_service.dart' show firebaseMessagingBackgroundHandler, initReminderPush, setupForegroundHandler;

/// ---- CONFIG ----

const String kDefaultServerAddr = 'jkazjsnynw.a.pinggy.link:10822';
const String kWebProxyUrl = 'https://localhost:8443';
const String _kPinggyApiKeyPref = 'pinggy_api_key';
const String _kVoiceInputEnabledPref = 'voice_input_enabled';
const String _kAutoSpeakEnabledPref = 'auto_speak_enabled';
const String _kReminderServerPref = 'reminder_server_url';

/// Pinggy Pro API: fetch active tunnel URL (requires Pro + API key from dashboard.pinggy.io/api-keys).
Future<String?> fetchPinggyTunnelUrl(String apiKey) async {
  if (apiKey.trim().isEmpty) return null;
  final resp = await http.get(
    Uri.parse('https://dashboard.pinggy.io/backend/api/v1/session/active'),
    headers: {'Authorization': 'Bearer ${apiKey.trim()}'},
  ).timeout(const Duration(seconds: 15));
  if (resp.statusCode != 200) return null;
  final list = jsonDecode(resp.body);
  if (list is! List || list.isEmpty) return null;
  // Prefer UDP (QUIC), else TCP; use most recent
  final sessions = list.cast<Map<String, dynamic>>();
  sessions.sort((a, b) {
    final aTs = a['lastUpdateTimestamp'] as String? ?? '';
    final bTs = b['lastUpdateTimestamp'] as String? ?? '';
    return bTs.compareTo(aTs);
  });
  for (final s in sessions) {
    final sub = s['subdomain'] as String?;
    final port = s['port'];
    if (sub != null && sub.isNotEmpty && port != null) {
      return '$sub:$port';
    }
  }
  return null;
}

/// Grok-style colors
const Color _bgDark = Color(0xFF0D0D0D);
const Color _surfaceDark = Color(0xFF1A1A1A);
const Color _inputDark = Color(0xFF141414);
const Color _textPrimary = Color(0xFFFFFFFF);
const Color _textSecondary = Color(0xB3FFFFFF);
const Color _accentBlue = Color(0xFF1DA1F2);

/// ---- EMBER CLIENT ----

class EmberClient {
  EmberClient(this._serverAddr, {this.proxyUrl});

  final String _serverAddr;
  final String? proxyUrl;

  Future<String> ask(String prompt) async {
    if (kIsWeb) {
      return askViaHttp(_serverAddr, prompt, proxyUrl: proxyUrl ?? kWebProxyUrl);
    }
    final response = await compute(_askOverQuic, [_serverAddr, prompt]);
    return response;
  }

  /// Streaming ask: yields tokens as they arrive (web) or full result at end (native).
  Stream<String> askStream(String prompt) async* {
    if (kIsWeb) {
      yield* askViaHttpStream(_serverAddr, prompt, proxyUrl: proxyUrl ?? kWebProxyUrl);
    } else {
      yield await compute(_askOverQuic, [_serverAddr, prompt]);
    }
  }
}

String _askOverQuic(List<String> args) => ask(args[0], args[1]);

String _sanitizeResponse(String s) {
  String out = s;
  const tags = [
    '<|im_end|>', '<|im_start|>', '<|end|>', '<|start|>',
    '<|thinking|>', '<|/thinking|>', '<|response|>', '<|/response|>',
    '<|system|>', '<|/system|>', '<|user|>', '<|/user|>',
    '<|assistant|>', '<|/assistant|>',
  ];
  for (final tag in tags) {
    out = out.replaceAll(tag, '');
  }
  while (true) {
    final start = out.indexOf('<|');
    if (start < 0) break;
    final end = out.indexOf('|>', start);
    if (end < 0) {
      out = out.substring(0, start);
      break;
    }
    out = '${out.substring(0, start)}${out.substring(end + 2)}';
  }
  return out.trim();
}

/// ---- CHAT MESSAGE ----

class _ChatMessage {
  final String text;
  final bool isUser;

  _ChatMessage(this.text, {this.isUser = false});
}

/// ---- GROK-STYLE APP ----

void main() async {
  WidgetsFlutterBinding.ensureInitialized();
  if (!kIsWeb) {
    try {
      await Firebase.initializeApp(options: DefaultFirebaseOptions.currentPlatform);
      FirebaseMessaging.onBackgroundMessage(firebaseMessagingBackgroundHandler);
      setupForegroundHandler();
    } catch (_) {
      // Firebase not configured - run without push
    }
  }
  runApp(const EmberApp());
}

class EmberApp extends StatelessWidget {
  const EmberApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Ember(push)',
      debugShowCheckedModeBanner: false,
      theme: ThemeData.dark().copyWith(
        scaffoldBackgroundColor: _bgDark,
        colorScheme: ColorScheme.dark(
          surface: _surfaceDark,
          onSurface: _textPrimary,
          primary: _accentBlue,
          onPrimary: Colors.white,
        ),
        appBarTheme: const AppBarTheme(
          backgroundColor: _surfaceDark,
          foregroundColor: _textPrimary,
          elevation: 0,
          centerTitle: false,
          titleTextStyle: TextStyle(
            color: _textPrimary,
            fontSize: 16,
            fontWeight: FontWeight.w600,
          ),
        ),
        inputDecorationTheme: InputDecorationTheme(
          filled: true,
          fillColor: _inputDark,
          border: OutlineInputBorder(borderRadius: BorderRadius.circular(20)),
          enabledBorder: OutlineInputBorder(
            borderRadius: BorderRadius.circular(20),
            borderSide: const BorderSide(color: Colors.transparent),
          ),
          focusedBorder: OutlineInputBorder(
            borderRadius: BorderRadius.circular(20),
            borderSide: BorderSide(color: _accentBlue.withValues(alpha: 0.5)),
          ),
          contentPadding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
          hintStyle: const TextStyle(color: _textSecondary, fontSize: 15),
        ),
      ),
      home: const EmberHomePage(),
    );
  }
}

class EmberHomePage extends StatefulWidget {
  const EmberHomePage({super.key});

  @override
  State<EmberHomePage> createState() => _EmberHomePageState();
}

class _EmberHomePageState extends State<EmberHomePage> with WidgetsBindingObserver {
  final _serverController = TextEditingController(text: kDefaultServerAddr);
  final _proxyController = TextEditingController(text: kWebProxyUrl);
  final _pinggyApiKeyController = TextEditingController();
  final _reminderServerController = TextEditingController();
  final _promptController = TextEditingController();
  final _scrollController = ScrollController();
  final _messages = <_ChatMessage>[];
  bool _loading = false;
  bool _showSettings = false;
  bool _refreshingPinggy = false;
  SharedPreferences? _prefs;

  // Voice
  final SpeechToText _speech = SpeechToText();
  final FlutterTts _tts = FlutterTts();
  bool _isListening = false;
  bool _speechAvailable = false;
  int? _speakingIndex;
  bool _voiceInputEnabled = true;
  bool _autoSpeakEnabled = false;

  // Location
  bool _gettingLocation = false;

  // Push polling (reminders, etc.)
  bool _pushPollingActive = false;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    _loadPrefs();
    _initSpeech();
    _initTts();
  }

  Future<void> _initSpeech() async {
    _speechAvailable = await _speech.initialize();
    if (mounted) setState(() {});
  }

  Future<void> _initTts() async {
    await _tts.setSpeechRate(0.5);
    await _tts.setVolume(1.0);
    await _tts.setPitch(1.0);
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.resumed) {
      _refreshPinggyUrlIfConfigured(_prefs?.getString(_kPinggyApiKeyPref) ?? _pinggyApiKeyController.text.trim());
      _fetchPushOnInit();
      _startPushPolling();
    } else if (state == AppLifecycleState.paused ||
        state == AppLifecycleState.inactive ||
        state == AppLifecycleState.detached) {
      _stopPushPolling();
    }
  }

  void _startPushPolling() {
    if (_pushPollingActive || kIsWeb) return;
    _pushPollingActive = true;
    _pushPollLoop();
  }

  void _stopPushPolling() {
    _pushPollingActive = false;
  }

  Future<void> _pushPollLoop() async {
    while (_pushPollingActive && mounted) {
      final addr = _serverController.text.trim();
      if (addr.isNotEmpty) {
        await _fetchPushOnInit();
      }
      if (!_pushPollingActive || !mounted) break;
      await Future.delayed(const Duration(seconds: 30));
    }
  }

  Future<void> _loadPrefs() async {
    final prefs = await SharedPreferences.getInstance();
    setState(() {
      _prefs = prefs;
      _pinggyApiKeyController.text = prefs.getString(_kPinggyApiKeyPref) ?? '';
      _reminderServerController.text = prefs.getString(_kReminderServerPref) ?? '';
      _voiceInputEnabled = prefs.getBool(_kVoiceInputEnabledPref) ?? true;
      _autoSpeakEnabled = prefs.getBool(_kAutoSpeakEnabledPref) ?? false;
    });
    await _refreshPinggyUrlIfConfigured(prefs.getString(_kPinggyApiKeyPref) ?? '');
    final reminderUrl = _reminderServerController.text.trim();
    if (reminderUrl.isNotEmpty && !kIsWeb) {
      final base = reminderUrl.contains('://') ? reminderUrl : 'https://$reminderUrl';
      initReminderPush(serverBaseUrl: base);
    }
    if (!kIsWeb) {
      _fetchPushOnInit();
      _startPushPolling();
    }
  }

  /// Fetch any queued push (e.g. reminder from push-queue) from Ember server.
  Future<void> _fetchPushOnInit() async {
    final addr = _serverController.text.trim();
    if (addr.isEmpty) return;
    try {
      final result = await compute(_askOverQuic, [addr, '__fetch_push__']);
      if (result.isNotEmpty && mounted) {
        try {
          final j = jsonDecode(result) as Map<String, dynamic>?;
          if (j != null && j['type'] == 'reminder') {
            final summary = j['summary'] as String? ?? 'Reminder';
            if (mounted) {
              ScaffoldMessenger.of(context).showSnackBar(
                SnackBar(content: Text('Reminder: $summary'), duration: const Duration(seconds: 5)),
              );
            }
          }
        } catch (_) {
          if (result.trim().isNotEmpty && mounted) {
            ScaffoldMessenger.of(context).showSnackBar(
              SnackBar(content: Text(result.length > 80 ? '${result.substring(0, 80)}...' : result)),
            );
          }
        }
      }
    } catch (_) {}
  }

  Future<void> _refreshPinggyUrlIfConfigured([String? apiKey]) async {
    final key = apiKey ?? _pinggyApiKeyController.text.trim();
    if (key.isEmpty) return;
    setState(() => _refreshingPinggy = true);
    try {
      final url = await fetchPinggyTunnelUrl(key);
      if (url != null && mounted) {
        _serverController.text = url;
        setState(() {});
      }
    } finally {
      if (mounted) setState(() => _refreshingPinggy = false);
    }
  }

  Future<void> _savePinggyApiKey() async {
    await _prefs?.setString(_kPinggyApiKeyPref, _pinggyApiKeyController.text.trim());
  }

  Future<void> _toggleListening() async {
    if (!_speechAvailable || _loading) return;
    if (_isListening) {
      await _speech.stop();
      setState(() => _isListening = false);
    } else {
      setState(() => _isListening = true);
      await _speech.listen(
        onResult: (result) {
          if (mounted) {
            _promptController.text = result.recognizedWords;
            setState(() {});
          }
        },
        listenFor: const Duration(seconds: 30),
        pauseFor: const Duration(seconds: 3),
        listenOptions: SpeechListenOptions(partialResults: true),
      );
    }
  }

  Future<void> _insertLocation() async {
    if (_loading || _gettingLocation || kIsWeb) return;
    setState(() => _gettingLocation = true);
    try {
      LocationPermission perm = await Geolocator.checkPermission();
      if (perm == LocationPermission.denied) {
        perm = await Geolocator.requestPermission();
      }
      if (perm == LocationPermission.denied || perm == LocationPermission.deniedForever) {
        if (mounted) {
          ScaffoldMessenger.of(context).showSnackBar(
            const SnackBar(content: Text('Location permission denied')),
          );
        }
        return;
      }
      final pos = await Geolocator.getCurrentPosition(
        locationSettings: const LocationSettings(accuracy: LocationAccuracy.medium),
      );
      if (mounted) {
        final loc = '[Location: ${pos.latitude.toStringAsFixed(5)}, ${pos.longitude.toStringAsFixed(5)}]';
        final current = _promptController.text.trim();
        _promptController.text = current.isEmpty ? loc : '$current $loc';
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Location error: $e')),
        );
      }
    } finally {
      if (mounted) setState(() => _gettingLocation = false);
    }
  }

  Future<void> _speakMessage(int index, String text) async {
    if (text.trim().isEmpty) return;
    if (_speakingIndex == index) {
      await _tts.stop();
      setState(() => _speakingIndex = null);
      return;
    }
    await _tts.stop();
    setState(() => _speakingIndex = index);
    _tts.setCompletionHandler(() {
      if (mounted) setState(() => _speakingIndex = null);
    });
    await _tts.speak(text);
  }

  Future<void> _send() async {
    final addr = _serverController.text.trim();
    final prompt = _promptController.text.trim();
    if (addr.isEmpty || prompt.isEmpty) return;

    setState(() {
      _messages.add(_ChatMessage(prompt, isUser: true));
      _messages.add(_ChatMessage('', isUser: false));
      _loading = true;
      _promptController.clear();
    });
    _scrollToBottom();

    try {
      final proxy = kIsWeb ? _proxyController.text.trim() : null;
      final client = EmberClient(addr, proxyUrl: proxy?.isNotEmpty == true ? proxy : null);
      var text = '';
      await for (final token in client.askStream(prompt)) {
        text += token;
        setState(() {
          _messages.removeLast();
          _messages.add(_ChatMessage(_sanitizeResponse(text), isUser: false));
        });
        _scrollToBottom();
      }
      final sanitized = _sanitizeResponse(text);
      setState(() {
        _messages.removeLast();
        _messages.add(_ChatMessage(sanitized, isUser: false));
        _loading = false;
      });
      if (_autoSpeakEnabled && sanitized.trim().isNotEmpty) {
        _speakMessage(_messages.length - 1, sanitized);
      }
    } catch (e) {
      setState(() {
        _messages.removeLast();
        _messages.add(_ChatMessage('Error: $e', isUser: false));
        _loading = false;
      });
    }
    _scrollToBottom();
  }

  void _scrollToBottom() {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (_scrollController.hasClients) {
        _scrollController.animateTo(
          _scrollController.position.maxScrollExtent,
          duration: const Duration(milliseconds: 200),
          curve: Curves.easeOut,
        );
      }
    });
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    _stopPushPolling();
    _speech.stop();
    _tts.stop();
    _serverController.dispose();
    _proxyController.dispose();
    _pinggyApiKeyController.dispose();
    _reminderServerController.dispose();
    _promptController.dispose();
    _scrollController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Ember(push)'),
        actions: [
          IconButton(
            icon: Icon(_showSettings ? Icons.close : Icons.settings_outlined),
            onPressed: () async {
              if (_showSettings) {
                await _prefs?.setString(_kReminderServerPref, _reminderServerController.text.trim());
                final url = _reminderServerController.text.trim();
                if (url.isNotEmpty && !kIsWeb) {
                  initReminderPush(serverBaseUrl: url.contains('://') ? url : 'https://$url');
                }
              }
              setState(() => _showSettings = !_showSettings);
            },
          ),
        ],
      ),
      body: Column(
        children: [
          if (_showSettings) _buildSettings(),
          Expanded(
            child: ListView.builder(
              controller: _scrollController,
              padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
              itemCount: _messages.length + (_loading ? 1 : 0),
              itemBuilder: (context, i) {
                if (i == _messages.length) {
                  return _buildMessageBubble('', isUser: false, streaming: true);
                }
                final msg = _messages[i];
                return _buildMessageBubble(msg.text, isUser: msg.isUser, index: i);
              },
            ),
          ),
          _buildInputBar(),
        ],
      ),
    );
  }

  Widget _buildSettings() {
    return Container(
      padding: const EdgeInsets.all(16),
      color: _surfaceDark,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          TextField(
            controller: _serverController,
            decoration: const InputDecoration(
              labelText: 'Ember server',
              hintText: '127.0.0.1:4433 or xxx.a.pinggy.link:port',
            ),
            style: const TextStyle(color: _textPrimary, fontSize: 14),
          ),
          const SizedBox(height: 12),
          Row(
            children: [
              Expanded(
                child: TextField(
                  controller: _pinggyApiKeyController,
                  obscureText: true,
                  decoration: const InputDecoration(
                    labelText: 'Pinggy API key (Pro)',
                    hintText: 'From dashboard.pinggy.io/api-keys',
                  ),
                  style: const TextStyle(color: _textPrimary, fontSize: 14),
                ),
              ),
              const SizedBox(width: 8),
              IconButton(
                onPressed: _refreshingPinggy ? null : () async {
                  await _savePinggyApiKey();
                  await _refreshPinggyUrlIfConfigured();
                },
                icon: _refreshingPinggy
                    ? const SizedBox(width: 20, height: 20, child: CircularProgressIndicator(strokeWidth: 2, color: _accentBlue))
                    : const Icon(Icons.refresh, color: _accentBlue),
                tooltip: 'Refresh Pinggy URL',
              ),
            ],
          ),
          const Padding(
            padding: EdgeInsets.only(top: 4),
            child: Text(
              'With a Pinggy Pro API key, the app fetches the current tunnel URL on start and when returning to the app.',
              style: TextStyle(color: _textSecondary, fontSize: 11),
            ),
          ),
          const SizedBox(height: 12),
          TextField(
            controller: _reminderServerController,
            decoration: const InputDecoration(
              labelText: 'Reminder push server (http_server URL)',
              hintText: 'https://your-tunnel:1235 or leave empty',
            ),
            style: const TextStyle(color: _textPrimary, fontSize: 14),
          ),
          const Padding(
            padding: EdgeInsets.only(top: 4),
            child: Text(
              'URL of http_server for reminder push. Must match the server running Promethos + reminder_worker.',
              style: TextStyle(color: _textSecondary, fontSize: 11),
            ),
          ),
          if (!kIsWeb) ...[
            const SizedBox(height: 16),
            const Divider(color: _textSecondary, height: 1),
            const SizedBox(height: 8),
            SwitchListTile(
              title: const Text('Voice input (mic)', style: TextStyle(color: _textPrimary, fontSize: 14)),
              subtitle: const Text('Show mic button for speech-to-text', style: TextStyle(color: _textSecondary, fontSize: 12)),
              value: _voiceInputEnabled,
              activeTrackColor: _accentBlue.withValues(alpha: 0.5),
              activeThumbColor: _accentBlue,
              onChanged: (v) async {
                setState(() => _voiceInputEnabled = v);
                await _prefs?.setBool(_kVoiceInputEnabledPref, v);
              },
            ),
            SwitchListTile(
              title: const Text('Auto-speak responses', style: TextStyle(color: _textPrimary, fontSize: 14)),
              subtitle: const Text('Read AI replies aloud automatically', style: TextStyle(color: _textSecondary, fontSize: 12)),
              value: _autoSpeakEnabled,
              activeTrackColor: _accentBlue.withValues(alpha: 0.5),
              activeThumbColor: _accentBlue,
              onChanged: (v) async {
                setState(() => _autoSpeakEnabled = v);
                await _prefs?.setBool(_kAutoSpeakEnabledPref, v);
                if (!v) await _tts.stop();
              },
            ),
          ],
          if (kIsWeb) ...[
            const SizedBox(height: 12),
            TextField(
              controller: _proxyController,
              decoration: const InputDecoration(
                labelText: 'HTTP proxy (web)',
                hintText: 'https://localhost:8443',
              ),
              style: const TextStyle(color: _textPrimary, fontSize: 14),
            ),
          ],
        ],
      ),
    );
  }

  Widget _buildMessageBubble(String text, {required bool isUser, bool streaming = false, int index = -1}) {
    final isSpeaking = !isUser && index >= 0 && _speakingIndex == index;
    return Padding(
      padding: const EdgeInsets.only(bottom: 12),
      child: Row(
        mainAxisAlignment: isUser ? MainAxisAlignment.end : MainAxisAlignment.start,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          if (!isUser)
            const Padding(
              padding: EdgeInsets.only(top: 4, right: 8),
              child: Icon(Icons.smart_toy_outlined, color: _accentBlue, size: 20),
            ),
          Flexible(
            child: Container(
              padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
              decoration: BoxDecoration(
                color: isUser ? _accentBlue.withValues(alpha: 0.2) : _surfaceDark,
                borderRadius: BorderRadius.only(
                  topLeft: const Radius.circular(18),
                  topRight: const Radius.circular(18),
                  bottomLeft: Radius.circular(isUser ? 18 : 4),
                  bottomRight: Radius.circular(isUser ? 4 : 18),
                ),
                border: Border.all(
                  color: isUser ? _accentBlue.withValues(alpha: 0.3) : Colors.transparent,
                  width: 1,
                ),
              ),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Row(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Expanded(
                        child: streaming
                            ? Row(
                                mainAxisSize: MainAxisSize.min,
                                children: [
                                  SizedBox(
                                    width: 16,
                                    height: 16,
                                    child: CircularProgressIndicator(
                                      strokeWidth: 2,
                                      color: _accentBlue,
                                    ),
                                  ),
                                  const SizedBox(width: 12),
                                  Text('Thinking...', style: TextStyle(color: _textSecondary, fontSize: 14)),
                                ],
                              )
                            : SelectableText(
                                text.isEmpty ? ' ' : text,
                                style: const TextStyle(
                                  color: _textPrimary,
                                  fontSize: 15,
                                  height: 1.5,
                                ),
                              ),
                      ),
                      if (!isUser && !streaming && text.trim().isNotEmpty)
                        IconButton(
                          iconSize: 20,
                          padding: EdgeInsets.zero,
                          constraints: const BoxConstraints(minWidth: 32, minHeight: 32),
                          onPressed: () => _speakMessage(index, text),
                          icon: Icon(
                            isSpeaking ? Icons.stop_circle : Icons.volume_up_outlined,
                            color: _accentBlue,
                            size: 20,
                          ),
                          tooltip: isSpeaking ? 'Stop' : 'Read aloud',
                        ),
                    ],
                  ),
                ],
              ),
            ),
          ),
          if (isUser) const SizedBox(width: 32),
        ],
      ),
    );
  }

  Widget _buildInputBar() {
    return Container(
      padding: const EdgeInsets.all(12),
      decoration: const BoxDecoration(color: _surfaceDark),
      child: SafeArea(
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.end,
          children: [
            if (!kIsWeb && _voiceInputEnabled && _speechAvailable) ...[
              IconButton(
                onPressed: _loading ? null : _toggleListening,
                icon: Icon(
                  _isListening ? Icons.stop : Icons.mic,
                  color: _isListening ? Colors.red : _accentBlue,
                  size: 24,
                ),
                tooltip: _isListening ? 'Stop listening' : 'Voice input',
              ),
              const SizedBox(width: 4),
            ],
            if (!kIsWeb) ...[
              IconButton(
                onPressed: _loading || _gettingLocation ? null : _insertLocation,
                icon: _gettingLocation
                    ? const SizedBox(width: 24, height: 24, child: CircularProgressIndicator(strokeWidth: 2, color: _accentBlue))
                    : const Icon(Icons.location_on_outlined, color: _accentBlue, size: 24),
                tooltip: 'Insert location',
              ),
              const SizedBox(width: 4),
            ],
            Expanded(
              child: TextField(
                controller: _promptController,
                maxLines: 4,
                minLines: 1,
                onSubmitted: (_) => _send(),
                decoration: InputDecoration(
                  hintText: _isListening ? 'Listening...' : 'Message Ember...',
                ),
                style: const TextStyle(color: _textPrimary, fontSize: 15),
              ),
            ),
            const SizedBox(width: 8),
            IconButton.filled(
              onPressed: _loading ? null : _send,
              icon: const Icon(Icons.arrow_upward, size: 22),
              style: IconButton.styleFrom(
                backgroundColor: _accentBlue,
                foregroundColor: Colors.white,
              ),
            ),
          ],
        ),
      ),
    );
  }
}
