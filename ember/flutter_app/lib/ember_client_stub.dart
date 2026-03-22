// Web: uses HTTP proxy (browsers cannot use QUIC).
// Run: cargo run --bin http_proxy -p ember-client --features http_proxy

import 'dart:async';
import 'dart:convert';

import 'package:http/http.dart' as http;

String ask(String serverAddr, String prompt) {
  throw UnsupportedError('Use EmberClient.ask() - it handles web via HTTP.');
}

/// Default proxy URL when not specified.
const String _defaultProxyUrl = 'https://localhost:8443';

/// Web-only: ask via HTTP proxy (blocking, full response).
Future<String> askViaHttp(String serverAddr, String prompt, {String? proxyUrl}) async {
  final base = proxyUrl ?? _defaultProxyUrl;
  final uri = Uri.parse('$base/ask');
  final body = jsonEncode({'server': serverAddr, 'prompt': prompt});
  final resp = await http
      .post(uri, body: body, headers: {'Content-Type': 'application/json'})
      .timeout(const Duration(seconds: 120));
  if (resp.statusCode != 200) {
    throw Exception('Proxy error ${resp.statusCode}: ${resp.body}');
  }
  return resp.body;
}

/// Web-only: streaming ask via HTTP proxy SSE (low latency).
Stream<String> askViaHttpStream(String serverAddr, String prompt, {String? proxyUrl}) async* {
  final base = proxyUrl ?? _defaultProxyUrl;
  final uri = Uri.parse('$base/ask/stream');
  final req = http.Request('POST', uri);
  req.body = jsonEncode({'server': serverAddr, 'prompt': prompt});
  req.headers['Content-Type'] = 'application/json';

  final client = http.Client();
  try {
    final resp = await client.send(req).timeout(const Duration(seconds: 120));
    if (resp.statusCode != 200) {
      throw Exception('Proxy error ${resp.statusCode}');
    }
    var buffer = '';
    await for (final chunk in resp.stream.transform(utf8.decoder)) {
      buffer += chunk;
      while (true) {
        final idx = buffer.indexOf('\n\n');
        if (idx < 0) break;
        final event = buffer.substring(0, idx);
        buffer = buffer.substring(idx + 2);
        if (event.startsWith('data: ')) {
          yield event.substring(6);
        }
      }
    }
  } finally {
    client.close();
  }
}
