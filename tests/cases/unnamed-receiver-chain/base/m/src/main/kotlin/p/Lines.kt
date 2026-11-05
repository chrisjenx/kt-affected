package p

class Tx {
    val id = 7
}

class Builder(private val assembler: Assembler) {
    fun build(tx: Tx): Int = with(tx) { assembler.assemble(id) }
}

class LineService(private val builder: Builder) {
    fun lines(tx: Tx): Int = with(tx) { builder.build(this) }
}

class Unrelated {
    fun lines(): Int = 3
}

object Wiring {
    fun service(): LineService = LineService(Builder(Assembler()))
    fun unrelated(): Unrelated = Unrelated()
}
