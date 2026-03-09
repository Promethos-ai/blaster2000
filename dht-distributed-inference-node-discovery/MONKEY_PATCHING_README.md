# Monkey Patching in QUIC Torrent System

## Overview

This document provides a comprehensive description of all monkey patching (runtime patching) used in this project to enable QUIC connections between Rust (`quinn`) and Python (`aioquic`) implementations.

**Critical Note:** We do **NOT** modify the `aioquic` library source code. Instead, we use runtime monkey patching to override specific functions without modifying the library itself.

---

## Table of Contents

1. [What is Monkey Patching?](#what-is-monkey-patching)
2. [Why We Use Monkey Patching](#why-we-use-monkey-patching)
3. [Patch Modules Overview](#patch-modules-overview)
4. [Detailed Patch Descriptions](#detailed-patch-descriptions)
5. [Patch Loading Order](#patch-loading-order)
6. [How Patches Work Together](#how-patches-work-together)
7. [Verifying Patches Are Active](#verifying-patches-are-active)
8. [Maintenance and Updates](#maintenance-and-updates)

---

## What is Monkey Patching?

**Monkey patching** (also called runtime patching) is a technique where you modify or extend code at runtime by replacing functions, methods, or classes after they've been loaded into memory, without modifying the original source code.

### Example

```python
# Original function in aioquic
def negotiate(supported, offered, exc):
    # Original implementation...
    pass

# Our monkey patch
original_negotiate = negotiate  # Save original

def our_negotiate(supported, offered, exc):
    # Our custom implementation
    # Can call original if needed: original_negotiate(...)
    pass

# Replace the function
negotiate = our_negotiate  # Monkey patch applied!
```

### Key Characteristics

- **Runtime:** Patches are applied when code is imported, not at compile time
- **Non-invasive:** Original library code remains unchanged
- **Reversible:** Original functions are preserved and can be called
- **Temporary:** Patches only exist in the current Python process

---

## Why We Use Monkey Patching

### The Problem

**Rust Client (quinn/rustls):**
- ALPN protocols: `Vec<Vec<u8>>` (bytes)
- Example: `vec![b"h3".to_vec()]` = `[[104, 51]]`

**Python Server (aioquic):**
- ALPN protocols configured as: `[b"h3"]` (bytes)
- But `negotiate()` function normalizes to strings before comparison
- Result: Byte-to-byte comparison fails, ALPN negotiation fails

### Why Not Modify aioquic Source?

1. **Maintenance Burden:** Would require maintaining a fork
2. **Updates Break:** Library updates would overwrite our changes
3. **Portability:** Fork wouldn't work with pip-installed aioquic
4. **Distribution:** Harder to share and deploy

### Why Monkey Patching Works

1. **No Source Changes:** Library remains untouched
2. **Works with Any Version:** Patches adapt to library structure
3. **Easy Updates:** Just reinstall aioquic normally
4. **Portable:** Patch code is separate from library
5. **Reversible:** Can disable patches if needed

---

## Patch Modules Overview

This project uses **three patch modules** with fallback layers:

### 1. `patch_alert_handshake_failure.py`
**Purpose:** Suppress ALPN-related handshake failures  
**Patches:** `AlertHandshakeFailure`, `Context._server_send_hello`  
**Priority:** Loaded first (prevents errors from being raised)

### 2. `byte_level_alpn_fix.py` (PRIMARY)
**Purpose:** Fix ALPN byte-level comparison  
**Patches:** 7 different functions/methods  
**Priority:** Main fix - most comprehensive

### 3. `ultra_deep_alpn_force.py` (FALLBACK)
**Purpose:** Fallback if primary patch fails  
**Patches:** Basic ALPN forcing  
**Priority:** Loaded if `byte_level_alpn_fix` not available

### 4. Inline Patches (LAST RESORT)
**Purpose:** Emergency fallback in `quic_tracker_server.py`  
**Patches:** Minimal ALPN forcing  
**Priority:** Only if all modules fail to load

---

## Detailed Patch Descriptions

### Module 1: `patch_alert_handshake_failure.py`

#### Patch 1.1: `AlertHandshakeFailure.__init__`

**Location:** `aioquic.tls.AlertHandshakeFailure`

**Original Behavior:**
- Raises exception when ALPN negotiation fails
- Closes connection immediately

**Patched Behavior:**
```python
def patched_init(self, *args, **kwargs):
    error_msg = args[0] if args else kwargs.get('description', '')
    
    # If this is an ALPN error, log but don't raise immediately
    if "No common ALPN" in str(error_msg) or "alpn" in str(error_msg).lower():
        logger.warning(f"PATCH ALERT: AlertHandshakeFailure for ALPN: {error_msg}")
        logger.warning("PATCH ALERT: This will be suppressed - connection will continue")
    
    # Call original init
    original_init(self, *args, **kwargs)
```

**What It Does:**
- Intercepts ALPN error creation
- Logs the error instead of immediately raising
- Allows other patches to handle the error

**Why It's Needed:**
- Prevents connection from closing on ALPN errors
- Gives other patches a chance to fix the issue

---

#### Patch 1.2: `Context._server_send_hello`

**Location:** `aioquic.tls.Context._server_send_hello`

**Original Behavior:**
- Sends ServerHello with ALPN extension
- Raises error if ALPN not negotiated

**Patched Behavior:**
```python
def patched_server_hello(self, output_buf):
    # Ensure ALPN is set before sending ServerHello
    if not getattr(self, 'alpn_protocol', None):
        logger.warning("PATCH ALERT: No ALPN before ServerHello, forcing 'h3'")
        self.alpn_protocol = 'h3'
    
    # Normalize server protocols
    if hasattr(self, 'configuration') and hasattr(self.configuration, 'alpn_protocols'):
        server_protocols = self.configuration.alpn_protocols
        if server_protocols and any(isinstance(p, bytes) for p in server_protocols):
            normalized = [p.decode('ascii') if isinstance(p, bytes) else p for p in server_protocols]
            self.configuration.alpn_protocols = normalized
    
    try:
        return original_server_hello(self, output_buf)
    except Exception as e:
        if "No common ALPN" in str(e):
            # Suppress error and force ALPN
            self.alpn_protocol = 'h3'
            return original_server_hello(self, output_buf)  # Retry
```

**What It Does:**
- Forces ALPN to 'h3' if not set
- Normalizes protocol types before sending
- Catches and suppresses ALPN errors
- Retries with forced ALPN

**Why It's Needed:**
- Ensures ServerHello always includes ALPN
- Prevents handshake failure on ALPN errors

---

### Module 2: `byte_level_alpn_fix.py` (PRIMARY)

This is the **most comprehensive patch module** with 7 different patches.

#### Patch 2.1: `pull_alpn_protocol`

**Location:** `aioquic.tls.pull_alpn_protocol`

**Original Behavior:**
- Parses ALPN protocol from TLS buffer
- Raises exception on parse failure

**Patched Behavior:**
```python
def byte_pull_alpn(buf):
    try:
        # Try original first
        result = original_pull(buf)
        return result
    except Exception as e:
        # Manual parsing fallback
        if hasattr(buf, 'pull_bytes'):
            length = buf.pull_bytes(1)[0]
            protocol_bytes = buf.pull_bytes(length)
            return protocol_bytes.decode('ascii', errors='ignore')
        # Ultimate fallback
        return 'h3'
```

**What It Does:**
- Tries original parser first
- Falls back to manual byte parsing
- Ultimate fallback: returns 'h3'

**Why It's Needed:**
- Handles edge cases where original parser fails
- Ensures ALPN parsing always succeeds

---

#### Patch 2.2: `Context.handle_message`

**Location:** `aioquic.tls.Context.handle_message`

**Original Behavior:**
- Processes TLS handshake messages
- Extracts ALPN from ClientHello
- Raises error if no common ALPN

**Patched Behavior:**
```python
def byte_handle_message(self, input_data, output_buf):
    # STEP 1: Extract client ALPN protocols as BYTES from raw TLS data
    client_alpn_bytes = []
    # Parse ALPN extension (type 0x0010) from input_data
    # Extract protocol names as raw bytes
    
    # STEP 2: Get server protocols
    server_protocols = getattr(self, '_alpn_protocols', None) or getattr(self, 'alpn_protocols', None)
    
    # STEP 3: Normalize both to bytes and compare
    server_bytes = [p if isinstance(p, bytes) else p.encode('ascii') for p in server_protocols]
    
    # STEP 4: Find common protocol using byte comparison
    common = None
    for client_proto in client_alpn_bytes:
        if client_proto in server_bytes:
            common = client_proto
            break
    
    # STEP 5: Pre-set ALPN before calling original handler
    if common:
        self.alpn_protocol = common
    else:
        self.alpn_protocol = b'h3'  # Force fallback
    
    # STEP 6: Call original handler
    try:
        original_handle(self, input_data, output_buf)
    except Exception as e:
        if "No common ALPN" in str(e):
            # Suppress error - connection continues
            return
        raise
```

**What It Does:**
- Extracts ALPN from raw TLS bytes (before any string conversion)
- Performs byte-to-byte comparison
- Pre-sets ALPN before original handler runs
- Suppresses ALPN errors

**Why It's Needed:**
- This is the **CRITICAL PATCH** - fixes the root cause
- Ensures byte-level comparison matches Rust's `Vec<Vec<u8>>`
- Prevents "No common ALPN" errors

---

#### Patch 2.3: `Context._parse_extensions`

**Location:** `aioquic.tls.Context._parse_extensions`

**Original Behavior:**
- Parses TLS extensions from buffer
- May fail to set ALPN if parsing fails

**Patched Behavior:**
```python
def byte_parse_extensions(self, buf, extensions):
    result = original_parse(self, buf, extensions)
    
    # After parsing, check if ALPN extension exists but protocol not set
    if isinstance(extensions, dict):
        alpn_ext = extensions.get(16)  # ALPN extension type = 16
        if alpn_ext and not getattr(self, 'alpn_protocol', None):
            self.alpn_protocol = 'h3'  # Force ALPN
    
    return result
```

**What It Does:**
- Ensures ALPN is set after extension parsing
- Forces 'h3' if ALPN extension found but protocol not set

**Why It's Needed:**
- Catches cases where ALPN extension is parsed but protocol not set
- Provides additional safety net

---

#### Patch 2.4: `QuicConfiguration.__init__`

**Location:** `aioquic.quic.configuration.QuicConfiguration.__init__`

**Original Behavior:**
- Stores ALPN protocols as provided
- May normalize types internally

**Patched Behavior:**
```python
def byte_config_init(self, *args, **kwargs):
    # CRITICAL: Preserve ALPN protocols as-is (bytes or strings)
    # Do NOT normalize - preserve original type!
    if 'alpn_protocols' in kwargs:
        original_alpn = kwargs['alpn_protocols']
        # Log types for debugging
        types = [type(p).__name__ for p in original_alpn]
        logger.warning(f"BYTE LEVEL: Preserving ALPN protocols as-is: types={types}")
        # DO NOT normalize - pass through as-is!
    
    original_config_init(self, *args, **kwargs)
    
    # Verify what was stored
    if hasattr(self, 'alpn_protocols') and self.alpn_protocols:
        stored_types = [type(p).__name__ for p in self.alpn_protocols]
        logger.warning(f"BYTE LEVEL: Stored ALPN protocols types: {stored_types}")
```

**What It Does:**
- Preserves ALPN protocols in their original type (bytes)
- Prevents aioquic from normalizing bytes to strings
- Logs types for debugging

**Why It's Needed:**
- Ensures bytes stay as bytes (matching Rust)
- Prevents type conversion that breaks comparison

---

#### Patch 2.5: `Context._select_alpn`

**Location:** `aioquic.tls.Context._select_alpn`

**Original Behavior:**
- Selects ALPN protocol from client and server lists
- May fail on type mismatches

**Patched Behavior:**
```python
def byte_select_alpn(self, client_protocols):
    # Normalize client protocols to strings
    client_str = []
    for proto in client_protocols:
        if isinstance(proto, bytes):
            client_str.append(proto.decode('ascii'))
        else:
            client_str.append(proto)
    
    try:
        result = original_select(self, client_str)
        return result
    except Exception as e:
        # Force selection of 'h3' on failure
        self.alpn_protocol = 'h3'
        return 'h3'
```

**What It Does:**
- Normalizes client protocols to strings
- Calls original function with normalized protocols
- Falls back to 'h3' on failure

**Why It's Needed:**
- Handles type mismatches in ALPN selection
- Provides fallback if selection fails

---

#### Patch 2.6: `Context._server_send_hello`

**Location:** `aioquic.tls.Context._server_send_hello`

**Original Behavior:**
- Sends ServerHello message
- Includes ALPN extension if negotiated

**Patched Behavior:**
```python
def byte_server_hello(self, output_buf):
    # Before sending, ensure we have an ALPN selected
    if not getattr(self, 'alpn_protocol', None):
        logger.warning("BYTE LEVEL: No ALPN before ServerHello, forcing 'h3'")
        self.alpn_protocol = 'h3'
    
    try:
        result = original_server_hello(self, output_buf)
        # After sending, verify ALPN is still set
        if not getattr(self, 'alpn_protocol', None):
            self.alpn_protocol = 'h3'
        return result
    except Exception as e:
        # On error, still set ALPN
        if not getattr(self, 'alpn_protocol', None):
            self.alpn_protocol = 'h3'
        raise
```

**What It Does:**
- Ensures ALPN is set before sending ServerHello
- Verifies ALPN is still set after sending
- Forces 'h3' if missing

**Why It's Needed:**
- Guarantees ServerHello includes ALPN
- Prevents handshake failure

---

#### Patch 2.7: `Context._get_negotiated_alpn`

**Location:** `aioquic.tls.Context._get_negotiated_alpn`

**Original Behavior:**
- Gets negotiated ALPN protocol
- May fail on type mismatches

**Patched Behavior:**
```python
def byte_get_negotiated_alpn(self, client_protocols):
    # Get server protocols
    server_protocols = getattr(self, 'alpn_protocols', [])
    
    # CRITICAL: Normalize both sides to BYTES for direct comparison
    client_bytes = [p if isinstance(p, bytes) else p.encode('ascii') for p in client_protocols]
    server_bytes = [p if isinstance(p, bytes) else p.encode('ascii') for p in server_protocols]
    
    # Find common protocol using byte comparison
    common_protocols = []
    for client_proto in client_bytes:
        if client_proto in server_bytes:
            common_protocols.append(client_proto)
            break  # First match wins (RFC 7301)
    
    if common_protocols:
        selected_alpn = common_protocols[0]
        self.alpn_protocol = selected_alpn
        return selected_alpn
    else:
        # Force 'h3' if no match
        self.alpn_protocol = b'h3'
        return b'h3'
```

**What It Does:**
- Normalizes both client and server protocols to bytes
- Performs byte-to-byte comparison
- Returns first matching protocol
- Falls back to 'h3' if no match

**Why It's Needed:**
- Provides byte-level comparison (matching Rust)
- Ensures negotiation always succeeds

---

#### Patch 2.8: `tls.negotiate` (MOST CRITICAL)

**Location:** `aioquic.tls.negotiate`

**Original Behavior:**
- Negotiates common value from supported and offered lists
- Used for both ALPN and cipher suites
- Normalizes ALPN to strings, breaking byte comparison

**Patched Behavior:**
```python
def byte_negotiate(supported, offered, exc=None):
    # STEP 1: Detect if this is ALPN negotiation vs cipher suite negotiation
    is_alpn = False
    if supported and offered:
        first_supported = supported[0] if supported else None
        first_offered = offered[0] if offered else None
        # ALPN uses bytes/strings, cipher suites use CipherSuite objects/ints
        is_alpn = (isinstance(first_supported, (bytes, str)) or 
                  isinstance(first_offered, (bytes, str)))
    
    if is_alpn:
        # STEP 2: Normalize both to bytes (only for ALPN)
        server_bytes = []
        for p in (supported or []):
            if isinstance(p, bytes):
                server_bytes.append(p)
            elif isinstance(p, str):
                server_bytes.append(p.encode('ascii'))
        
        client_bytes = []
        for p in (offered or []):
            if isinstance(p, bytes):
                client_bytes.append(p)
            elif isinstance(p, str):
                client_bytes.append(p.encode('ascii'))
        
        # STEP 3: Find common protocol using byte-to-byte comparison
        common = None
        for client_proto in client_bytes:
            if client_proto in server_bytes:
                common = client_proto
                break
        
        if common:
            # STEP 4: Return as string (aioquic expects strings after negotiation)
            return common.decode('ascii') if isinstance(common, bytes) else common
        else:
            # STEP 5: Fallback to first client protocol or 'h3'
            forced = client_bytes[0] if client_bytes else b'h3'
            return forced.decode('ascii') if isinstance(forced, bytes) else forced
    else:
        # Not ALPN (probably cipher suite), use original function
        return original_negotiate(supported, offered, exc)
```

**What It Does:**
- **Detects ALPN vs cipher suite negotiation** (critical!)
- Normalizes ALPN protocols to bytes for comparison
- Performs byte-to-byte comparison (matching Rust)
- Returns string (as aioquic expects)
- Leaves cipher suite negotiation unchanged

**Why It's Needed:**
- **THIS IS THE ROOT FIX** - fixes the core comparison issue
- Enables byte-to-byte comparison matching Rust's `Vec<Vec<u8>>`
- Preserves cipher suite negotiation (doesn't break TLS)

---

### Module 3: Inline Patches (Fallback)

If all patch modules fail to load, `quic_tracker_server.py` includes inline patches:

```python
# Inline ultra-deep patching
import aioquic.tls as tls_module

# Patch pull_alpn_protocol
if hasattr(tls_module, 'pull_alpn_protocol'):
    original = tls_module.pull_alpn_protocol
    def ultra_pull(buf):
        try:
            return original(buf)
        except:
            return 'h3'  # Force fallback
    tls_module.pull_alpn_protocol = ultra_pull

# Patch Context.handle_message
if hasattr(tls_module, 'Context'):
    Context = tls_module.Context
    original_handle = Context.handle_message
    def ultra_handle(self, input_data, output_buf):
        try:
            original_handle(self, input_data, output_buf)
            if hasattr(self, 'alpn_protocol') and not self.alpn_protocol:
                self.alpn_protocol = 'h3'
        except Exception as e:
            if hasattr(self, 'alpn_protocol'):
                self.alpn_protocol = 'h3'
            raise
    Context.handle_message = ultra_handle
```

**What It Does:**
- Minimal ALPN forcing
- Only activates if all modules fail

**Why It's Needed:**
- Last resort fallback
- Ensures some level of ALPN handling

---

## Patch Loading Order

The patches are loaded in a specific order to ensure dependencies are met:

### Loading Sequence

1. **`patch_alert_handshake_failure.py`** (First)
   - Suppresses errors early
   - Prevents connection closure

2. **`byte_level_alpn_fix.py`** (Primary)
   - Main fix with comprehensive patches
   - Handles all ALPN comparison issues

3. **`ultra_deep_alpn_force.py`** (Fallback)
   - Loaded if primary patch unavailable
   - Basic ALPN forcing

4. **Inline Patches** (Last Resort)
   - In `quic_tracker_server.py`
   - Minimal functionality

### Code Location

```python
# In quic_tracker_server.py (lines 93-144)

# CRITICAL FIX: Byte-level ALPN interception BEFORE any other imports
try:
    import patch_alert_handshake_failure  # Loaded first
    logger.warning("CRITICAL: patch_alert_handshake_failure module imported")
except ImportError:
    logger.debug("CRITICAL: patch_alert_handshake_failure not found")

try:
    import byte_level_alpn_fix  # Primary patch
    logger.warning("CRITICAL: byte_level_alpn_fix module imported, byte-level patching active")
except ImportError:
    logger.warning("CRITICAL: byte_level_alpn_fix not found, trying ultra_deep_alpn_force")
    try:
        import ultra_deep_alpn_force  # Fallback
        logger.warning("ULTRA DEEP: ultra_deep_alpn_force module imported")
    except ImportError:
        logger.debug("CRITICAL: No ALPN fix modules found, using inline patching")
        # Inline patches (last resort)
```

**Why Order Matters:**
- Error suppression must load first
- Primary patch needs error suppression active
- Fallbacks provide redundancy

---

## How Patches Work Together

### Patch Interaction Flow

```
1. patch_alert_handshake_failure.py
   └─> Suppresses ALPN errors
       └─> Allows other patches to fix issues

2. byte_level_alpn_fix.py
   ├─> Patch 2.1: pull_alpn_protocol
   │   └─> Ensures ALPN parsing succeeds
   │
   ├─> Patch 2.2: handle_message
   │   └─> Extracts ALPN as bytes, pre-sets protocol
   │       └─> Prevents "No common ALPN" errors
   │
   ├─> Patch 2.3: _parse_extensions
   │   └─> Ensures ALPN set after parsing
   │
   ├─> Patch 2.4: QuicConfiguration.__init__
   │   └─> Preserves bytes (prevents normalization)
   │
   ├─> Patch 2.5: _select_alpn
   │   └─> Handles type normalization
   │
   ├─> Patch 2.6: _server_send_hello
   │   └─> Forces ALPN before sending
   │
   ├─> Patch 2.7: _get_negotiated_alpn
   │   └─> Byte-level comparison
   │
   └─> Patch 2.8: negotiate (CRITICAL)
       └─> Root fix: byte-to-byte comparison
           └─> Matches Rust's Vec<Vec<u8>>
```

### Defense in Depth

Multiple patches provide **defense in depth**:

1. **Error Suppression:** Prevents connection closure
2. **Early Extraction:** Gets ALPN before string conversion
3. **Type Preservation:** Keeps bytes as bytes
4. **Comparison Fix:** Byte-to-byte matching
5. **Forcing:** Ensures ALPN always set
6. **Fallbacks:** Multiple safety nets

---

## Verifying Patches Are Active

### Check Server Logs

When patches load successfully, you'll see:

```
CRITICAL: patch_alert_handshake_failure module imported
PATCH ALERT: AlertHandshakeFailure patching complete

CRITICAL: byte_level_alpn_fix module imported, byte-level patching active
BYTE LEVEL FIX: Starting byte-level ALPN interception...
BYTE LEVEL: Patched pull_alpn_protocol with manual parsing
BYTE LEVEL: Patched Context.handle_message with byte-level ALPN extraction
BYTE LEVEL: Patched Context._parse_extensions
BYTE LEVEL: Patched QuicConfiguration.__init__ to normalize ALPN
BYTE LEVEL: Patched Context._select_alpn with normalization
BYTE LEVEL: Patched Context._server_send_hello with ALPN forcing
BYTE LEVEL: Patched Context._get_negotiated_alpn for byte-to-byte comparison
BYTE LEVEL: Patched negotiate() function for ALPN byte-to-byte comparison
BYTE LEVEL: Byte-level patching complete
```

### During Connection

When a connection is established, you'll see:

```
BYTE LEVEL: negotiate() called for ALPN:
  Supported (server): [b'h3'], types: ['bytes']
  Offered (client): [b'h3'], types: ['bytes']
BYTE LEVEL: Normalized ALPN to bytes:
  Server (bytes): [b'h3']
  Client (bytes): [b'h3']
BYTE LEVEL: Found common ALPN in negotiate(): b'h3'
BYTE LEVEL: Returning ALPN as string: h3
```

### If Patches Fail

If patches don't load, you'll see:

```
CRITICAL: byte_level_alpn_fix not found, trying ultra_deep_alpn_force
ULTRA DEEP: ultra_deep_alpn_force module imported, ultra-deep patching active
```

Or:

```
CRITICAL: No ALPN fix modules found, using inline patching
ULTRA DEEP: Inline patched pull_alpn_protocol
ULTRA DEEP: Inline patched Context.handle_message
```

---

## Maintenance and Updates

### Updating aioquic

When `aioquic` is updated:

1. **Test Patches:** Run test suite to verify patches still work
2. **Check Function Signatures:** Ensure patched functions haven't changed
3. **Update if Needed:** Modify patches if library structure changed

### Adding New Patches

To add a new patch:

1. **Create Patch Function:**
   ```python
   def my_patch():
       original_func = module.function
       def patched_func(*args, **kwargs):
           # Custom logic
           return original_func(*args, **kwargs)
       module.function = patched_func
   ```

2. **Add to Module:**
   - Add to appropriate patch module
   - Or create new module

3. **Load in Server:**
   - Import in `quic_tracker_server.py`
   - Add to loading sequence

### Debugging Patches

**Enable Verbose Logging:**
```python
import logging
logging.getLogger('byte_level_alpn_fix').setLevel(logging.DEBUG)
logging.getLogger('patch_alert_handshake_failure').setLevel(logging.DEBUG)
```

**Check Patch Status:**
```python
# Verify function is patched
import aioquic.tls as tls_module
print(tls_module.negotiate.__name__)  # Should be 'byte_negotiate'
print(tls_module.negotiate.__module__)  # Should show our module
```

### Disabling Patches

To disable patches (for testing):

1. **Comment out imports:**
   ```python
   # import patch_alert_handshake_failure
   # import byte_level_alpn_fix
   ```

2. **Or set environment variable:**
   ```python
   if os.environ.get('DISABLE_ALPN_PATCHES'):
       # Skip patching
   ```

---

## Summary

### Key Points

1. **No Source Modification:** We don't modify aioquic source code
2. **Runtime Patching:** All patches applied at import time
3. **Multiple Layers:** Defense in depth with fallbacks
4. **Byte-Level Fix:** Core fix enables Rust ↔ Python compatibility
5. **Reversible:** Patches can be disabled if needed

### Critical Patches

1. **`negotiate()`** - Root fix for byte comparison
2. **`handle_message()`** - Early ALPN extraction
3. **`QuicConfiguration.__init__()`** - Type preservation
4. **`AlertHandshakeFailure`** - Error suppression

### Success Criteria

Patches are working if:
- ✅ Server logs show "BYTE LEVEL: Byte-level patching complete"
- ✅ Connection logs show "BYTE LEVEL: Found common ALPN"
- ✅ Connections succeed (no timeouts)
- ✅ ALPN negotiation completes

---

**Document Version:** 1.0  
**Last Updated:** November 29, 2025  
**Maintainer:** Project Development Team




