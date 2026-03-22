// FFI bindings to ember_native (Rust QUIC client).
// Requires libember_native.so in jniLibs (Android) or linked (iOS).

import 'dart:ffi';
import 'dart:io';

import 'package:ffi/ffi.dart';

typedef EmberAskNative = Pointer<Utf8> Function(
    Pointer<Utf8> serverAddr, Pointer<Utf8> prompt);
typedef EmberAskDart = Pointer<Utf8> Function(
    Pointer<Utf8> serverAddr, Pointer<Utf8> prompt);

typedef EmberFreeStringNative = Void Function(Pointer<Utf8> ptr);
typedef EmberFreeStringDart = void Function(Pointer<Utf8> ptr);

DynamicLibrary? _lib;
EmberAskDart? _emberAsk;
EmberFreeStringDart? _emberFreeString;

void _loadLibrary() {
  if (_lib != null) return;
  if (Platform.isAndroid) {
    _lib = DynamicLibrary.open('libember_native.so');
  } else if (Platform.isIOS) {
    _lib = DynamicLibrary.process();
  } else {
    throw UnsupportedError('Ember FFI only supports Android and iOS');
  }
  _emberAsk = _lib!
      .lookup<NativeFunction<EmberAskNative>>('ember_ask')
      .asFunction();
  _emberFreeString = _lib!
      .lookup<NativeFunction<EmberFreeStringNative>>('ember_free_string')
      .asFunction();
}

/// Ask the AI via the ember server. Blocks until response.
String ask(String serverAddr, String prompt) {
  _loadLibrary();
  final addrPtr = serverAddr.toNativeUtf8();
  final promptPtr = prompt.toNativeUtf8();
  try {
    final resultPtr = _emberAsk!(addrPtr, promptPtr);
    if (resultPtr == nullptr) return 'Error: null response';
    final result = resultPtr.toDartString();
    _emberFreeString!(resultPtr);
    return result;
  } finally {
    malloc.free(addrPtr);
    malloc.free(promptPtr);
  }
}
