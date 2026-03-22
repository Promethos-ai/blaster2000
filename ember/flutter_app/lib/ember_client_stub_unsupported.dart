// Placeholder for native: askViaHttp and askViaHttpStream are web-only, never called on native.

Future<String> askViaHttp(String serverAddr, String prompt, {String? proxyUrl}) {
  throw UnsupportedError('askViaHttp is web-only');
}

Stream<String> askViaHttpStream(String serverAddr, String prompt, {String? proxyUrl}) {
  throw UnsupportedError('askViaHttpStream is web-only');
}
