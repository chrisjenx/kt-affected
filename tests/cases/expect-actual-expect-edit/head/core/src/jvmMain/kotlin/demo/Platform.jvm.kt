package demo

actual fun platformName(short: Boolean): String = if (short) "J" else "JVM"
