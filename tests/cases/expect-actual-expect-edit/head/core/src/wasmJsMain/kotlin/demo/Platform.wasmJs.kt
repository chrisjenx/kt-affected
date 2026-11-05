package demo

actual fun platformName(short: Boolean): String = if (short) "W" else "Web"
