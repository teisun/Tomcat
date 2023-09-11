package com.tomcat.service;

import com.tomcat.controller.requeset.ProfileReq;
import com.tomcat.controller.response.ProfileResp;

public interface UserProfileService {

  ProfileResp getByUserId(String userId);

  ProfileResp update(ProfileReq resquest);

}