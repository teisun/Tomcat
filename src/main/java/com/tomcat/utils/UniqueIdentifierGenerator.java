package com.tomcat.utils;

import java.util.UUID;

/**
 * @projectName: Tomcat
 * @package: com.tomcat.utils
 * @className: UniqueIdentifierGenerator
 * @author: tomcat
 * @description: TODO
 * @date: 2023/9/11 10:05 AM
 * @version: 1.0
 */
public class UniqueIdentifierGenerator {
    public static String uniqueId(){
        UUID uuid = UUID.randomUUID();
        String uniqueId = uuid.toString();
        System.out.println("Generated UUID: " + uniqueId);
        return uniqueId;
    }
}
