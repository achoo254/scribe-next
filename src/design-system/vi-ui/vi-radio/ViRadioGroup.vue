<script setup lang="ts">
// ViRadioGroup — wrapper iNET ViUI quanh v-radio-group. Chứa ViRadio (vd tiêu chí cắt Phút/MB/Chia-K).
defineOptions({ inheritAttrs: false });
</script>

<template>
  <v-radio-group v-bind="$attrs">
    <template v-for="(_, name) in $slots" #[name]="slotProps">
      <slot :name="name" v-bind="slotProps ?? {}" />
    </template>
  </v-radio-group>
</template>
