package com.tomcat.controller.requeset;

import lombok.AllArgsConstructor;
import lombok.Data;

@Data
@AllArgsConstructor
public class ProfileReq {

    private String userId;

    private String nativeLanguage;

    private String depth;

    private String style;

    private String targetLanguage;

}
