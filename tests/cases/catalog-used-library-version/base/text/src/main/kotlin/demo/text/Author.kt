package demo.text

import okio.ByteString.Companion.encodeUtf8

class Author(val name: String) {
    fun digest(): String = name.encodeUtf8().sha256().hex()
}
