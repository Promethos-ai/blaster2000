// Platform-specific ember client.
// Native (Android/iOS): QUIC via FFI.
// Web: stub (QUIC not supported in browsers).

export 'ember_ffi.dart' if (dart.library.html) 'ember_client_stub.dart';
