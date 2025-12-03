<template>
  <div>Hello from Component</div>
</template>

<script setup>
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
