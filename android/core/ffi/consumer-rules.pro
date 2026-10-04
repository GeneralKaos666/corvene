# JNA and the UniFFI bindings, for whichever app shrinks this library.
# JNA reaches its native side by reflection and JNI callbacks.
-keep class com.sun.jna.** { *; }
-keep class * implements com.sun.jna.** { *; }
-keepclassmembers class * extends com.sun.jna.Structure { <fields>; <methods>; }
-dontwarn java.awt.**
-dontwarn com.sun.jna.**
# The bindings: the library interface JNA proxies, the callback structures
# the engine calls back through (HostEvents), and the records' fields.
-keep class com.wasimaster.corvene.ffi.gen.** { *; }
-keepclasseswithmembernames class * { native <methods>; }
