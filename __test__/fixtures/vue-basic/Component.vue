<template>
  <div>Hello from Component</div>
</template>

<script setup lang="ts">
export const useCounter = () => {
  const count = 1;
  return { count };
};

export const useStore = () => {
  return { state: {} };
};

export const unusedHelper = () => {
  return 'unused';
};
</script>
