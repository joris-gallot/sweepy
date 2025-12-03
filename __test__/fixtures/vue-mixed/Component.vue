<template>
  <div>
    <p>{{ message }}</p>
  </div>
</template>

<script setup>
import { helper } from './utils';

export const useComponent = () => {
  const message = helper('hello');
  return { message };
};

export const ComponentName = 'Component';
</script>
