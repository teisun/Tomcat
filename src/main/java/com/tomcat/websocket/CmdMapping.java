package com.tomcat.websocket;

import java.lang.annotation.ElementType;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;

/**
 * @description websocket命令映射注解
 * @author tomcat
 * @date 2023/9/18 2:19 PM
 */
@Target(ElementType.METHOD)
@Retention(RetentionPolicy.RUNTIME)
public @interface CmdMapping {
    String value() default "";
}
