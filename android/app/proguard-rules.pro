# Corvene release shrinking. :core:ffi's consumer-rules.pro keeps JNA and
# the UniFFI bindings; this file adds what only the application knows.

# Navigation 3 keys are restored from saved state by their serializers.
-keepclassmembers @kotlinx.serialization.Serializable class com.wasimaster.corvene.** {
    *** Companion;
    kotlinx.serialization.KSerializer serializer(...);
}
-keep,includedescriptorclasses class com.wasimaster.corvene.**$$serializer { *; }

# Backtraces from the field should name real lines.
-keepattributes SourceFile,LineNumberTable
-renamesourcefileattribute SourceFile
