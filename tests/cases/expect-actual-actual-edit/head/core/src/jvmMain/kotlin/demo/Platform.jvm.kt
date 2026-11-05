package demo

actual fun platformName(): String = "JVM " + System.getProperty("java.version")
