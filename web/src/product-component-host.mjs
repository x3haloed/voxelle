const AsyncFunction = Object.getPrototypeOf(async function () {}).constructor;

export class ProductComponentHost {
  constructor(api, documentObject = document) {
    this.api = api;
    this.document = documentObject;
    this.activeDigest = "";
    this.disposeActive = null;
    this.activeComponent = null;
    this.activeStyle = null;
  }

  compile(component) {
    if (component.api_version !== 1) {
      throw new Error(`unsupported product component API ${component.api_version}`);
    }
    return new AsyncFunction(
      "api",
      `"use strict";\n${component.source}\n//# sourceURL=voxelle-product-component-${component.digest}.js`,
    );
  }

  style(component) {
    const style = this.document.createElement("style");
    style.dataset.voxelleProductComponent = component.digest;
    style.textContent = component.styles;
    return style;
  }

  async activate(component) {
    if (component.digest === this.activeDigest) return false;
    const factory = this.compile(component);
    const previousDispose = this.disposeActive;
    const previousComponent = this.activeComponent;
    const previousStyle = this.activeStyle;
    await previousDispose?.();
    previousStyle?.remove();
    const nextStyle = this.style(component);
    this.document.head.append(nextStyle);
    try {
      const nextDispose = await factory(this.api);
      if (typeof nextDispose !== "function") {
        throw new Error("product component did not return a disposer");
      }
      this.disposeActive = nextDispose;
      this.activeComponent = component;
      this.activeStyle = nextStyle;
      this.activeDigest = component.digest;
      return true;
    } catch (error) {
      nextStyle.remove();
      if (previousComponent) {
        this.document.head.append(previousStyle);
        this.disposeActive = await this.compile(previousComponent)(this.api);
        this.activeComponent = previousComponent;
        this.activeStyle = previousStyle;
        this.activeDigest = previousComponent.digest;
      }
      throw error;
    }
  }
}
