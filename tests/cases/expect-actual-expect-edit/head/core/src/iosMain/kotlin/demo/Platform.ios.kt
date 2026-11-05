package demo

actual fun platformName(short: Boolean): String = if (short) "i" else "iOS"
