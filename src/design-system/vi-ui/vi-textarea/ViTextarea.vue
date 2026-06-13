<script setup lang="ts">
// ViTextarea — wrapper iNET ViUI quanh v-textarea. Panel transcript (readonly + copy).
defineOptions({ inheritAttrs: false });
</script>

<template>
  <v-textarea variant="outlined" v-bind="$attrs">
    <template v-for="(_, name) in $slots" #[name]="slotProps">
      <slot :name="name" v-bind="slotProps ?? {}" />
    </template>
  </v-textarea>
</template>
