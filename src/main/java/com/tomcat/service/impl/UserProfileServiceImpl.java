package com.tomcat.service.impl;

import com.tomcat.controller.requeset.ProfileReq;
import com.tomcat.controller.response.ProfileResp;
import com.tomcat.domain.User;
import com.tomcat.domain.UserProfile;
import com.tomcat.domain.UserProfileRepository;
import com.tomcat.domain.UserRepository;
import com.tomcat.service.UserProfileService;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.stereotype.Service;

@Service
public class UserProfileServiceImpl implements UserProfileService {
  
  @Autowired
  private UserProfileRepository userProfileRepository;

  @Autowired
  UserRepository userRepository;

  @Override
  public ProfileResp getByUserId(String userId) {
    UserProfile userProfile = userProfileRepository.findByUserId(userId);
    ProfileResp response;
    if(userProfile != null) response = ProfileResp.build(userProfile);
    else {
      response = new ProfileResp();
      response.setUserId(userId);

    }
    return response;
  }

  @Override
  public ProfileResp update(ProfileReq profileRequest) {

    User user = userRepository.findById(profileRequest.getUserId())
            .orElseThrow(() -> new RuntimeException("用户不存在"));

    UserProfile profile = new UserProfile();
    profile.setUser(user);
    profile.setMotherTongue(profileRequest.getMotherTongue());
    profile.setCommunicationStyle(profileRequest.getCommunicationStyle());
    profile.setLanguageDepth(profileRequest.getLanguageDepth());
    profile.setTargetLanguage(profileRequest.getTargetLanguage());

    UserProfile save = userProfileRepository.save(profile);
    ProfileResp response = ProfileResp.build(save);


    return response;
  }

}