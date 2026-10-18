export const fileName = (path: string) => {
  return path.split(/[\\/]/).pop() ?? path;
};
