package demo

class Literal {
    fun greeting(): String = "hello!"
}

class ParamName {
    fun greet(person: String): String = "hi $person"
}

class InferredType {
    fun ratio(): Int = 1
}

class Untouched {
    fun same(): Int = 0
}
