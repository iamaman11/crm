declare module "*.module.css" {
  const classes: Readonly<Record<string,string>>;
  export default classes;
}

// Global styles are bundled for side effects; they have no JavaScript exports.
declare module "*.css" {}
