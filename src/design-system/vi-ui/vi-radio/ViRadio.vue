<script setup lang="ts">
// ViRadio — wrapper iNET ViUI quanh v-radio. Một lựa chọn trong ViRadioGroup.
defineOptions({ inheritAttrs: false });
</script>

<template>
  <v-radio v-bind="$attrs">
    <template v-for="(_, name) in $slots" #[name]="slotProps">
      <slot :name="name" v-bind="slotProps ?? {}" />
    </template>
  </v-radio>
</template>
