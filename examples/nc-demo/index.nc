{
  "component": "page",
  "title": "Nefu .nc Component Demo",
  "theme": "light",
  "children": [
    {
      "component": "navbar",
      "brand": "NC Demo",
      "theme": "dark",
      "bg": "primary",
      "items": [
        { "label": "Home", "href": "#home", "active": true },
        { "label": "Components", "href": "#components" },
        { "label": "Forms", "href": "#forms" },
        { "label": "Data", "href": "#data" },
        { "label": "About", "href": "#about" }
      ]
    },
    {
      "component": "container",
      "fluid": true,
      "class": "py-4",
      "children": [
        {
          "component": "alert",
          "type": "info",
          "dismissible": true,
          "text": "🎉 Welcome to the Nefu .nc component demo! This page showcases 15+ built-in components."
        },
        {
          "component": "row",
          "children": [
            {
              "component": "col",
              "size": 8,
              "children": [
                {
                  "component": "card",
                  "title": "Buttons & Badges",
                  "class": "mb-4",
                  "children": [
                    {
                      "component": "button",
                      "text": "Primary Button",
                      "color": "primary",
                      "size": "md",
                      "onClick": "lj('btnClick', 'primary')"
                    },
                    {
                      "component": "button",
                      "text": "Success Button",
                      "color": "success",
                      "outline": true,
                      "onClick": "lj('btnClick', 'success')"
                    },
                    {
                      "component": "button",
                      "text": "Danger Button",
                      "color": "danger",
                      "disabled": false,
                      "onClick": "lj('btnClick', 'danger')"
                    },
                    {
                      "component": "button",
                      "text": "Loading...",
                      "color": "warning",
                      "loading": true
                    },
                    { "component": "spacer" },
                    {
                      "component": "badge",
                      "text": "New Release",
                      "color": "primary",
                      "pill": true
                    },
                    {
                      "component": "badge",
                      "text": "3",
                      "color": "danger",
                      "pill": true
                    },
                    {
                      "component": "badge",
                      "text": "Beta",
                      "color": "warning"
                    }
                  ]
                },
                {
                  "component": "card",
                  "title": "Progress Bars",
                  "class": "mb-4",
                  "children": [
                    {
                      "component": "progress",
                      "value": 75,
                      "max": 100,
                      "color": "primary",
                      "striped": true,
                      "animated": true,
                      "label": "75%"
                    },
                    {
                      "component": "progress",
                      "value": 45,
                      "max": 100,
                      "color": "success",
                      "label": "45% Complete"
                    },
                    {
                      "component": "progress",
                      "value": 90,
                      "max": 100,
                      "color": "danger",
                      "striped": true
                    }
                  ]
                },
                {
                  "component": "tabs",
                  "id": "demo-tabs",
                  "tabs": [
                    {
                      "id": "tab-overview",
                      "label": "Overview",
                      "active": true,
                      "children": [
                        {
                          "component": "text",
                          "content": "This is the tab content area. Nefu's .nc language lets you describe UI with JSON without writing HTML by hand.",
                          "class": "p-3"
                        }
                      ]
                    },
                    {
                      "id": "tab-features",
                      "label": "Features",
                      "children": [
                        {
                          "component": "list",
                          "items": [
                            "30+ built-in Bootstrap 5 components",
                            "JSON format, easy to generate and maintain",
                            "Supports event binding and bridge calls",
                            "Real-time preview and hot reload"
                          ],
                          "class": "p-3"
                        }
                      ]
                    },
                    {
                      "id": "tab-api",
                      "label": "API",
                      "children": [
                        {
                          "component": "code",
                          "language": "javascript",
                          "content": "// Bind events in .nc\n\"onClick\": \"lj('myMethod', arg1)\"\n\n// Equivalent to\nnefu.invoke('myMethod', arg1)",
                          "class": "p-3"
                        }
                      ]
                    }
                  ]
                }
              ]
            },
            {
              "component": "col",
              "size": 4,
              "children": [
                {
                  "component": "card",
                  "title": "Quick Info",
                  "class": "mb-4",
                  "children": [
                    {
                      "component": "list-group",
                      "items": [
                        { "text": "Version", "badge": "1.0.0" },
                        { "text": "Runtime", "badge": "Go" },
                        { "text": "Encryption", "badge": "AES-256" },
                        { "text": "Compression", "badge": "ZIP" },
                        { "text": "Components", "badge": "30+" }
                      ]
                    }
                  ]
                },
                {
                  "component": "card",
                  "title": "System Status",
                  "class": "mb-4",
                  "children": [
                    {
                      "component": "progress",
                      "value": 62,
                      "max": 100,
                      "color": "info",
                      "label": "Memory 62%"
                    },
                    {
                      "component": "progress",
                      "value": 35,
                      "max": 100,
                      "color": "success",
                      "label": "CPU 35%"
                    },
                    {
                      "component": "progress",
                      "value": 80,
                      "max": 100,
                      "color": "warning",
                      "label": "Disk 80%"
                    }
                  ]
                },
                {
                  "component": "accordion",
                  "id": "faq-accordion",
                  "items": [
                    {
                      "id": "faq-1",
                      "title": "What is a .nc file?",
                      "expanded": true,
                      "children": [
                        {
                          "component": "text",
                          "content": ".nc stands for Nefu Component, a JSON-based UI description language."
                        }
                      ]
                    },
                    {
                      "id": "faq-2",
                      "title": "How do I bind events?",
                      "children": [
                        {
                          "component": "text",
                          "content": "Use attributes like onClick and onChange, with lj() or nefu.invoke() call expressions as values."
                        }
                      ]
                    },
                    {
                      "id": "faq-3",
                      "title": "Are custom components supported?",
                      "children": [
                        {
                          "component": "text",
                          "content": "Yes. You can embed any HTML through the html component, or inject raw markup using the raw component."
                        }
                      ]
                    }
                  ]
                }
              ]
            }
          ]
        },
        {
          "component": "card",
          "title": "Form Example",
          "id": "forms",
          "class": "mt-4",
          "children": [
            {
              "component": "form",
              "id": "demo-form",
              "onSubmit": "lj('formSubmit', this)",
              "children": [
                {
                  "component": "row",
                  "children": [
                    {
                      "component": "col",
                      "size": 6,
                      "children": [
                        {
                          "component": "input",
                          "id": "username",
                          "label": "Username",
                          "type": "text",
                          "placeholder": "Please enter your username",
                          "required": true
                        }
                      ]
                    },
                    {
                      "component": "col",
                      "size": 6,
                      "children": [
                        {
                          "component": "input",
                          "id": "email",
                          "label": "Email Address",
                          "type": "email",
                          "placeholder": "name@example.com",
                          "required": true
                        }
                      ]
                    }
                  ]
                },
                {
                  "component": "select",
                  "id": "role",
                  "label": "Select Role",
                  "options": [
                    { "value": "", "label": "-- Please select --", "disabled": true },
                    { "value": "admin", "label": "Administrator" },
                    { "value": "editor", "label": "Editor" },
                    { "value": "viewer", "label": "Viewer", "selected": true }
                  ]
                },
                {
                  "component": "textarea",
                  "id": "bio",
                  "label": "Biography",
                  "rows": 3,
                  "placeholder": "Tell us about yourself..."
                },
                {
                  "component": "checkbox",
                  "id": "agree",
                  "label": "I agree to the Terms of Service and Privacy Policy",
                  "required": true
                },
                {
                  "component": "radio-group",
                  "id": "contact-method",
                  "label": "Preferred Contact Method",
                  "options": [
                    { "value": "email", "label": "Email" },
                    { "value": "phone", "label": "Phone" },
                    { "value": "wechat", "label": "WeChat" }
                  ],
                  "inline": true
                },
                {
                  "component": "button",
                  "text": "Submit Form",
                  "color": "primary",
                  "type": "submit"
                },
                {
                  "component": "button",
                  "text": "Reset",
                  "color": "secondary",
                  "type": "reset",
                  "outline": true
                }
              ]
            }
          ]
        },
        {
          "component": "card",
          "title": "Data Table",
          "id": "data",
          "class": "mt-4",
          "children": [
            {
              "component": "table",
              "striped": true,
              "hover": true,
              "bordered": true,
              "responsive": true,
              "headers": ["ID", "Name", "Department", "Position", "Status", "Actions"],
              "rows": [
                {
                  "cells": ["1001", "Zhang San", "Engineering", "Senior Engineer", "Active"],
                  "actions": [
                    { "label": "Edit", "color": "primary", "onClick": "lj('editUser', 1001)" },
                    { "label": "Delete", "color": "danger", "onClick": "lj('deleteUser', 1001)" }
                  ]
                },
                {
                  "cells": ["1002", "Li Si", "Product", "Product Manager", "Active"],
                  "actions": [
                    { "label": "Edit", "color": "primary", "onClick": "lj('editUser', 1002)" },
                    { "label": "Delete", "color": "danger", "onClick": "lj('deleteUser', 1002)" }
                  ]
                },
                {
                  "cells": ["1003", "Wang Wu", "Design", "UI Designer", "On Leave"],
                  "actions": [
                    { "label": "Edit", "color": "primary", "onClick": "lj('editUser', 1003)" },
                    { "label": "Delete", "color": "danger", "onClick": "lj('deleteUser', 1003)" }
                  ]
                },
                {
                  "cells": ["1004", "Zhao Liu", "Marketing", "Marketing Director", "Active"],
                  "actions": [
                    { "label": "Edit", "color": "primary", "onClick": "lj('editUser', 1004)" },
                    { "label": "Delete", "color": "danger", "onClick": "lj('deleteUser', 1004)" }
                  ]
                },
                {
                  "cells": ["1005", "Qian Qi", "HR", "HR Specialist", "Left"],
                  "actions": [
                    { "label": "Edit", "color": "primary", "onClick": "lj('editUser', 1005)" },
                    { "label": "Delete", "color": "danger", "onClick": "lj('deleteUser', 1005)" }
                  ]
                }
              ]
            }
          ]
        },
        {
          "component": "modal",
          "id": "demo-modal",
          "title": "Modal Demo",
          "size": "lg",
          "children": [
            {
              "component": "text",
              "content": "This is a modal defined with .nc. It supports various sizes and content combinations."
            },
            {
              "component": "alert",
              "type": "warning",
              "text": "Other components can also be nested inside a modal!"
            }
          ],
          "footer": [
            {
              "component": "button",
              "text": "Close",
              "color": "secondary",
              "dismiss": true
            },
            {
              "component": "button",
              "text": "Confirm",
              "color": "primary",
              "onClick": "lj('modalConfirm')"
            }
          ]
        },
        {
          "component": "row",
          "class": "mt-4",
          "children": [
            {
              "component": "col",
              "size": 4,
              "children": [
                {
                  "component": "card",
                  "class": "text-center",
                  "children": [
                    {
                      "component": "icon",
                      "name": "bi-speedometer2",
                      "size": 3,
                      "color": "primary"
                    },
                    {
                      "component": "heading",
                      "level": 5,
                      "text": "High Performance"
                    },
                    {
                      "component": "text",
                      "content": "Native WebView rendering for fast startup"
                    }
                  ]
                }
              ]
            },
            {
              "component": "col",
              "size": 4,
              "children": [
                {
                  "component": "card",
                  "class": "text-center",
                  "children": [
                    {
                      "component": "icon",
                      "name": "bi-shield-lock",
                      "size": 3,
                      "color": "success"
                    },
                    {
                      "component": "heading",
                      "level": 5,
                      "text": "Secure & Reliable"
                    },
                    {
                      "component": "text",
                      "content": "AES-256-GCM encryption protects your source code"
                    }
                  ]
                }
              ]
            },
            {
              "component": "col",
              "size": 4,
              "children": [
                {
                  "component": "card",
                  "class": "text-center",
                  "children": [
                    {
                      "component": "icon",
                      "name": "bi-puzzle",
                      "size": 3,
                      "color": "warning"
                    },
                    {
                      "component": "heading",
                      "level": 5,
                      "text": "Flexible & Extensible"
                    },
                    {
                      "component": "text",
                      "content": "Bridge API makes it easy to call system features"
                    }
                  ]
                }
              ]
            }
          ]
        },
        {
          "component": "separator"
        },
        {
          "component": "text",
          "content": "© 2024 Nefu Project — Built with the .nc component language",
          "class": "text-center text-muted py-3"
        }
      ]
    }
  ]
}
