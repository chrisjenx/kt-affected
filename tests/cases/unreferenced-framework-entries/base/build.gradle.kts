tasks.wrapper {
    doLast { demo.plugins.WrapperStanza.inject(file("gradlew")) }
}
